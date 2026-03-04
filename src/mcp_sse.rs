// MCP over SSE 传输层实现
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    http::StatusCode,
};
use futures::stream::Stream;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::models::AppState;

pub async fn mcp_sse_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let _guard = state.sse_pool.try_acquire()
        .map_err(|_| {
            tracing::warn!("SSE 连接数已达上限");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    
    let session_id = Uuid::new_v4().to_string();
    tracing::info!("MCP SSE 连接建立 (session_id: {}, 当前连接数: {})", 
        session_id, state.sse_pool.current_count());
    
    let config = state.config.read().await;
    let channel_capacity = config.http_mcp_channel_capacity;
    let heartbeat_interval = config.http_sse_heartbeat;
    drop(config);
    
    let (tx, rx) = mpsc::channel(channel_capacity);
    
    state.mcp_sessions.add_session(session_id.clone(), tx.clone());
    
    // 1. 发送 endpoint 事件
    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("/mcp?session={}", session_id));
    
    let _ = tx.send(Ok(endpoint_event)).await;
    
    // 2. 自动发送 initialize 响应
    let initialize_response = json!({
        "jsonrpc": "2.0",
        "id": null,
        "result": {
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
        }
    });
    
    let initialize_event = Event::default()
        .event("message")
        .data(initialize_response.to_string());
    
    if let Err(e) = tx.send(Ok(initialize_event)).await {
        tracing::error!("发送 initialize 响应失败: {}", e);
    } else {
        tracing::debug!("自动发送 initialize 响应 (session: {})", session_id);
    }
    
    // 3. 自动发送 tools/list 响应
    let tools_list_response = json!({
        "jsonrpc": "2.0",
        "id": null,
        "result": {
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
        }
    });
    
    let tools_event = Event::default()
        .event("message")
        .data(tools_list_response.to_string());
    
    if let Err(e) = tx.send(Ok(tools_event)).await {
        tracing::error!("发送 tools/list 响应失败: {}", e);
    } else {
        tracing::debug!("自动发送 tools/list 响应 (session: {})", session_id);
    }
    
    // 4. 启动心跳任务
    let sessions = state.mcp_sessions.clone();
    let session_id_clone = session_id.clone();
    
    tokio::spawn(async move {
        let mut ping_count = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(heartbeat_interval)).await;
            ping_count += 1;
            
            let ping_event = Event::default()
                .event("ping")
                .data(json!({"timestamp": chrono::Utc::now().to_rfc3339(), "count": ping_count}).to_string());
            
            if tx.send(Ok(ping_event)).await.is_err() {
                tracing::info!("MCP SSE 连接已关闭 (session_id: {}, 存活时间: {}秒)", 
                    session_id_clone, ping_count * heartbeat_interval);
                sessions.remove_session(&session_id_clone);
                break;
            }
            
            tracing::debug!("发送心跳 #{} (session: {})", ping_count, session_id_clone);
        }
    });
    
    Ok(Sse::new(ReceiverStream::new(rx))
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text("keep-alive")
        ))
}
