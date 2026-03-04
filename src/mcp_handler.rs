// MCP 协议处理器
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::error_codes;
use crate::models::{AppState, JsonRpcRequest, JsonRpcResponse, JsonRpcError};

fn error_response(id: Option<serde_json::Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message }),
    }
}

pub async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    Json(request): Json<JsonRpcRequest>,
) -> Result<StatusCode, (StatusCode, Json<JsonRpcResponse>)> {
    let request_id = uuid::Uuid::new_v4();
    
    let session_id = params.get("session")
        .ok_or_else(|| {
            tracing::warn!("MCP 请求失败: 缺少 session 参数 [{}]", request_id);
            (StatusCode::BAD_REQUEST, Json(error_response(
                request.id.clone(),
                error_codes::INVALID_REQUEST,
                "Missing session parameter".to_string()
            )))
        })?;
    
    tracing::debug!("MCP 请求: method={}, session={}, id={:?} [{}]", 
        request.method, session_id, request.id, request_id);
    
    let tx = state.mcp_sessions.get_session(session_id)
        .ok_or_else(|| {
            tracing::warn!("MCP 请求失败: session 不存在 [{}]", request_id);
            (StatusCode::BAD_REQUEST, Json(error_response(
                request.id.clone(),
                error_codes::SESSION_NOT_FOUND,
                "Session not found".to_string()
            )))
        })?;
    
    // 对于 tools/call，传递 tx 以便发送进度事件
    let response = if request.method == "tools/call" {
        handle_mcp_request_with_progress(&state, request, tx.clone()).await
    } else {
        handle_mcp_request(request).await
    };
    
    let event_data = match serde_json::to_string(&response) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("序列化 MCP 响应失败: {} [{}]", e, request_id);
            return Ok(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    let event = axum::response::sse::Event::default()
        .event("message")
        .data(event_data);
    
    if let Err(e) = tx.send(Ok(event)).await {
        tracing::error!("发送 MCP 响应失败: {} [{}]", e, request_id);
        return Ok(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    tracing::debug!("MCP 响应已推送 [{}]", request_id);
    Ok(StatusCode::ACCEPTED)
}

async fn handle_mcp_request(request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "tools/list" => handle_tools_list(request),
        "tools/call" => panic!("tools/call should use handle_mcp_request_with_progress"),
        "resources/list" => handle_resources_list(request),
        "resources/read" => handle_resources_read(request),
        "prompts/list" => handle_prompts_list(request),
        "prompts/get" => handle_prompts_get(request),
        _ => error_response(
            request.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method)
        ),
    }
}

async fn handle_mcp_request_with_progress(
    state: &AppState, 
    request: JsonRpcRequest,
    tx: tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>
) -> JsonRpcResponse {
    match request.method.as_str() {
        "tools/call" => handle_tools_call_with_progress(state, request, tx).await,
        _ => handle_mcp_request(request).await,
    }
}

fn handle_initialize(request: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            },
            "serverInfo": {
                "name": "ai-search-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        error: None,
    }
}

fn handle_tools_list(request: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(json!({
            "tools": [
                {
                    "name": "ai_search",
                    "description": "Search using AI-powered multi-dimensional query analysis",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query"
                            }
                        },
                        "required": ["query"]
                    }
                }
            ]
        })),
        error: None,
    }
}

#[allow(dead_code)]
async fn handle_tools_call(state: &AppState, request: JsonRpcRequest) -> JsonRpcResponse {
    let params = request.params.as_object();
    
    if params.is_none() {
        return error_response(request.id, error_codes::INVALID_PARAMS, "Invalid params".to_string());
    }
    
    let params = params.unwrap();
    let tool_name = params.get("name").and_then(|v| v.as_str());
    let arguments = params.get("arguments");
    
    if tool_name != Some("ai_search") {
        return error_response(
            request.id,
            error_codes::INVALID_PARAMS,
            format!("Unknown tool: {:?}", tool_name)
        );
    }
    
    let query = arguments
        .and_then(|a| a.get("query"))
        .and_then(|q| q.as_str());
    
    if query.is_none() {
        return error_response(request.id, error_codes::INVALID_PARAMS, "Missing query parameter".to_string());
    }
    
    let query = query.unwrap();
    
    if query.is_empty() || query.len() > 10000 {
        return error_response(
            request.id,
            error_codes::INVALID_PARAMS,
            "Query must be between 1 and 10000 characters".to_string()
        );
    }
    
    let start = std::time::Instant::now();
    
    let ai_client = state.ai_client.read().await;
    match ai_client.search(query).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let config = state.config.read().await;
            
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": result
                        }
                    ],
                    "metadata": {
                        "duration_ms": duration_ms,
                        "model": config.search_model_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                })),
                error: None,
            }
        }
        Err(e) => error_response(
            request.id,
            error_codes::SEARCH_ERROR,
            format!("Search failed: {}", e)
        ),
    }
}

async fn handle_tools_call_with_progress(
    state: &AppState, 
    request: JsonRpcRequest,
    tx: tokio::sync::mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>
) -> JsonRpcResponse {
    let params = request.params.as_object();
    
    if params.is_none() {
        return error_response(request.id, error_codes::INVALID_PARAMS, "Invalid params".to_string());
    }
    
    let params = params.unwrap();
    let tool_name = params.get("name").and_then(|v| v.as_str());
    let arguments = params.get("arguments");
    
    if tool_name != Some("ai_search") {
        return error_response(
            request.id,
            error_codes::INVALID_PARAMS,
            format!("Unknown tool: {:?}", tool_name)
        );
    }
    
    let query = arguments
        .and_then(|a| a.get("query"))
        .and_then(|q| q.as_str());
    
    if query.is_none() {
        return error_response(request.id, error_codes::INVALID_PARAMS, "Missing query parameter".to_string());
    }
    
    let query = query.unwrap();
    
    if query.is_empty() || query.len() > 10000 {
        return error_response(
            request.id,
            error_codes::INVALID_PARAMS,
            "Query must be between 1 and 10000 characters".to_string()
        );
    }
    
    let start = std::time::Instant::now();
    
    // 启动进度心跳任务，每 5 秒发送一次进度事件
    let tx_clone = tx.clone();
    let progress_task = tokio::spawn(async move {
        let mut count = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            count += 1;
            
            // 发送 SSE 注释作为进度指示（不会被客户端当作消息处理）
            // 注意：不要手动添加 \n，Axum 会自动处理
            let comment = format!("progress {} searching...", count * 5);
            let progress_event = axum::response::sse::Event::default()
                .comment(&comment);
            
            if tx_clone.send(Ok(progress_event)).await.is_err() {
                break;
            }
            
            tracing::debug!("发送工具执行进度 #{}", count);
        }
    });
    
    let ai_client = state.ai_client.read().await;
    let result = ai_client.search(query).await;
    
    // 取消进度任务
    progress_task.abort();
    
    match result {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let config = state.config.read().await;
            
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": result
                        }
                    ],
                    "metadata": {
                        "duration_ms": duration_ms,
                        "model": config.search_model_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }
                })),
                error: None,
            }
        }
        Err(e) => error_response(
            request.id,
            error_codes::SEARCH_ERROR,
            format!("Search failed: {}", e)
        ),
    }
}

fn handle_resources_list(request: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(json!({
            "resources": []
        })),
        error: None,
    }
}

fn handle_resources_read(request: JsonRpcRequest) -> JsonRpcResponse {
    error_response(
        request.id,
        error_codes::INVALID_PARAMS,
        "No resources available".to_string()
    )
}

fn handle_prompts_list(request: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: request.id,
        result: Some(json!({
            "prompts": []
        })),
        error: None,
    }
}

fn handle_prompts_get(request: JsonRpcRequest) -> JsonRpcResponse {
    error_response(
        request.id,
        error_codes::INVALID_PARAMS,
        "No prompts available".to_string()
    )
}
