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

// MCP 协议处理器（SSE 模式）
pub async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    Json(request): Json<JsonRpcRequest>,
) -> StatusCode {
    let request_id = uuid::Uuid::new_v4();
    
    let session_id = match params.get("session") {
        Some(id) => id,
        None => {
            tracing::warn!("MCP SSE 请求失败: 缺少 session 参数");
            return StatusCode::BAD_REQUEST;
        }
    };
    
    tracing::info!("MCP SSE 请求: method={}, session={}, id={:?} [{}]", 
        request.method, session_id, request.id, request_id);
    
    let tx = match state.mcp_sessions.get_session(session_id) {
        Some(tx) => tx,
        None => {
            tracing::warn!("MCP SSE 请求失败: session 不存在");
            return StatusCode::BAD_REQUEST;
        }
    };
    
    // 异步处理请求并通过 SSE 推送响应
    let state_clone = state.clone();
    let tx_clone = tx.clone();
    let method = request.method.clone();
    
    tokio::spawn(async move {
        // 对于 tools/call，传递 tx 以便发送进度事件
        let response = if method == "tools/call" {
            handle_mcp_request_with_progress(&state_clone, request, tx_clone.clone()).await
        } else {
            handle_mcp_request(request).await
        };
        
        tracing::info!("MCP SSE 响应: method={}, status={}, id={:?} [{}]", 
            response.jsonrpc, 
            if response.error.is_some() { "error" } else { "success" },
            response.id, 
            request_id);
        
        // 通过 SSE 推送响应（标准 MCP SSE 模式）
        let response_json = serde_json::to_string(&response).unwrap_or_default();
        let sse_event = axum::response::sse::Event::default()
            .event("message")
            .data(response_json);
        
        if let Err(e) = tx_clone.send(Ok(sse_event)).await {
            tracing::warn!("通过 SSE 推送响应失败: {}", e);
        }
    });
    
    // 立即返回 202 Accepted（响应将通过 SSE 异步推送）
    StatusCode::ACCEPTED
}

// MCP 协议处理器（HTTP 模式 - 支持 Cherry Studio 的 streamableHttp）
pub async fn mcp_http_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, (StatusCode, Json<JsonRpcResponse>)> {
    let request_id = uuid::Uuid::new_v4();
    
    tracing::info!("MCP HTTP 请求: method={}, id={:?} [{}]", 
        request.method, request.id, request_id);
    
    // 对于非 tools/call 请求，同步返回
    // 对于 tools/call，也同步返回（Cherry Studio 会等待）
    let response = if request.method == "tools/call" {
        handle_tools_call(&state, request).await
    } else {
        handle_mcp_request(request).await
    };
    
    tracing::info!("MCP HTTP 响应: method={}, status={}, id={:?} [{}]", 
        response.jsonrpc, 
        if response.error.is_some() { "error" } else { "success" },
        response.id, 
        request_id);
    
    // 总是返回 200 OK，错误信息在 JSON 中
    Ok(Json(response))
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
            
            tracing::trace!("发送工具执行进度 #{}", count);
        }
    });
    
    let ai_client = state.ai_client.read().await;
    let result = ai_client.search_with_progress(query, Some(tx.clone())).await;
    
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
