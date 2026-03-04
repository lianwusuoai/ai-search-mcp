use axum::{
    extract::{Query, State},
    response::{sse::Event, Sse},
    Json,
};
use chrono::Utc;
use futures::stream::Stream;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::models::{AppError, AppState, HealthResponse, SearchRequest, SearchResponse};
use crate::sse::{heartbeat_task, SseEventBuilder};

// 健康检查处理器

pub async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let uptime = state.start_time.elapsed().as_secs();
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "ai-search-mcp-server".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    })
}

// HTTP JSON 搜索处理器

pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(_params): Query<HashMap<String, String>>,
    Json(payload): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    // 验证请求
    let query = payload.query.trim();
    
    if query.is_empty() {
        tracing::warn!("空查询");
        return Err(AppError::InvalidRequest("查询不能为空".to_string()));
    }
    
    // 从配置读取查询长度限制
    let config = state.config.read().await;
    let max_query_length = config.max_query_length;
    
    if query.len() > max_query_length {
        tracing::warn!("查询过长: {} 字符", query.len());
        return Err(AppError::InvalidRequest(
            format!("查询长度不能超过 {} 字符", max_query_length)
        ));
    }
    
    // 检查潜在的恶意内容
    if query.contains("<script>") || query.contains("javascript:") {
        tracing::warn!("检测到潜在恶意内容");
        return Err(AppError::InvalidRequest("查询包含非法字符".to_string()));
    }
    
    // 执行搜索
    let start = Instant::now();
    let ai_client = state.ai_client.read().await;
    match ai_client.search(query).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::debug!("搜索完成: {} ms", duration_ms);
            
            // 判断是否有子查询
            let sub_queries = if config.max_query_plan > 1 {
                Some(config.max_query_plan as usize)
            } else {
                None
            };
            
            Ok(Json(SearchResponse {
                result,
                query: payload.query,
                duration_ms,
                model: config.search_model_id.clone(),
                timestamp: Utc::now().to_rfc3339(),
                sub_queries,
            }))
        }
        Err(e) => {
            tracing::error!("搜索失败: {}", e);
            Err(AppError::SearchError(e.to_string()))
        }
    }
}

// SSE 流式搜索处理器

pub async fn search_stream_handler(
    State(state): State<Arc<AppState>>,
    Query(_params): Query<HashMap<String, String>>,
    Json(payload): Json<SearchRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    // 验证请求
    let query = payload.query.trim();
    
    if query.is_empty() {
        tracing::warn!("空查询");
        return Err(AppError::InvalidRequest("查询不能为空".to_string()));
    }
    
    // 从配置读取查询长度限制
    let config = state.config.read().await;
    let max_query_length = config.max_query_length;
    
    if query.len() > max_query_length {
        tracing::warn!("查询过长: {} 字符", query.len());
        return Err(AppError::InvalidRequest(
            format!("查询长度不能超过 {} 字符", max_query_length)
        ));
    }
    
    // 检查潜在的恶意内容
    if query.contains("<script>") || query.contains("javascript:") {
        tracing::warn!("检测到潜在恶意内容");
        return Err(AppError::InvalidRequest("查询包含非法字符".to_string()));
    }
    
    // 创建事件流
    let (tx, rx) = mpsc::channel(100);
    
    // 克隆需要的数据
    let ai_client = state.ai_client.read().await.clone();
    let config_clone = config.clone();
    let query_owned = query.to_string();
    let search_tx = tx.clone();
    
    tokio::spawn(async move {
        // 推送开始事件
        let _ = search_tx.send(Ok(SseEventBuilder::start(&query_owned))).await;
        
        let start = Instant::now();
        
        // 执行搜索（使用带进度的版本）
        match ai_client.search_with_progress(&query_owned, Some(search_tx.clone())).await {
            Ok(result) => {
                // 推送结果事件
                let _ = search_tx.send(Ok(SseEventBuilder::result(0, &result))).await;
                
                // 推送完成事件
                let duration_ms = start.elapsed().as_millis() as u64;
                let _ = search_tx.send(Ok(SseEventBuilder::complete(
                    1,
                    duration_ms,
                    &config_clone.search_model_id,
                    &Utc::now().to_rfc3339(),
                ))).await;
            }
            Err(e) => {
                // 推送错误事件
                tracing::error!("SSE 搜索失败: {}", e);
                let _ = search_tx.send(Ok(SseEventBuilder::error(
                    &e.to_string(),
                    "SEARCH_ERROR",
                ))).await;
            }
        }
    });
    
    // 启动心跳任务
    let heartbeat_interval = config.http_sse_heartbeat;
    tokio::spawn(heartbeat_task(tx, heartbeat_interval));
    
    // 返回事件流
    Ok(Sse::new(ReceiverStream::new(rx)))
}
