use axum::{
    extract::State,
    http::{Request, StatusCode, Method, HeaderMap},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

// CORS 中间件

pub fn cors_layer() -> CorsLayer {
    use tower_http::cors::AllowOrigin;
    
    // 从环境变量读取允许的域名，默认拒绝所有（生产环境安全）
    let allow_origin = if let Ok(origins) = std::env::var("AI_CORS_ORIGINS") {
        if origins.to_lowercase() == "any" {
            // 显式设置为 "any" 才允许所有域名（开发模式）
            tracing::warn!("CORS 允许所有域名（仅用于开发环境）");
            AllowOrigin::any()
        } else {
            // 生产环境：限制特定域名
            let origins: Vec<_> = origins
                .split(',')
                .filter_map(|s| s.trim().parse::<axum::http::HeaderValue>().ok())
                .collect();
            
            if origins.is_empty() {
                tracing::error!("CORS 配置为空，拒绝所有跨域请求");
                AllowOrigin::list(vec![])
            } else {
                tracing::info!("CORS 限制域名: {:?}", origins);
                AllowOrigin::list(origins)
            }
        }
    } else {
        // 未设置环境变量：默认只允许本地访问
        tracing::info!("未设置 AI_CORS_ORIGINS，仅允许本地访问");
        let local_origins = vec![
            "http://localhost".parse().unwrap(),
            "http://127.0.0.1".parse().unwrap(),
        ];
        AllowOrigin::list(local_origins)
    };
    
    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-request-id"),
        ])
        .max_age(std::time::Duration::from_secs(3600))
}

// API Key 认证中间件

pub async fn auth_middleware(
    State(state): State<Arc<crate::models::AppState>>,
    headers: HeaderMap,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path();
    
    // 健康检查端点不需要认证
    if path == "/health" {
        return Ok(next.run(request).await);
    }
    
    // /mcp 端点通过 session_id 验证（在 mcp_handler 中处理）
    if path == "/mcp" {
        return Ok(next.run(request).await);
    }
    
    // 配置界面相关端点使用 cookie 认证（在 config_ui.rs 中处理）
    // 不需要 API key 认证
    if path == "/" || path == "/login" || path.starts_with("/static/") || path.starts_with("/api/auth/") || path.starts_with("/api/config") || path.starts_with("/api/defaults") || path.starts_with("/api/test") || path.starts_with("/api/restart") {
        return Ok(next.run(request).await);
    }
    
    // /sse 端点需要 API key 认证（支持 query 参数以兼容 SSE）
    if path == "/sse" {
        // SSE 连接可能无法设置自定义 header，保留 query 参数支持
        let query = request.uri().query().unwrap_or("");
        if let Some(key_value) = query.split('&')
            .find(|param| param.starts_with("key="))
            .and_then(|param| param.strip_prefix("key=")) {
            let config = state.config.read().await;
            if key_value == config.http_api_key {
                return Ok(next.run(request).await);
            }
        }
        tracing::warn!("SSE 认证失败: API key 无效或缺失");
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 其他端点使用 Authorization header 认证
    let auth_header = headers.get("Authorization")
        .and_then(|v| v.to_str().ok());
    
    if auth_header.is_none() {
        tracing::warn!("认证失败: 缺少 Authorization header");
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let auth_value = auth_header.unwrap();
    
    // 支持 "Bearer <token>" 格式
    let api_key = if auth_value.starts_with("Bearer ") {
        auth_value.strip_prefix("Bearer ").unwrap()
    } else {
        auth_value
    };
    
    let config = state.config.read().await;
    if api_key != config.http_api_key {
        tracing::warn!("认证失败: API key 不匹配");
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(next.run(request).await)
}

// 请求日志中间件

pub async fn logging_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();
    
    tracing::info!("请求: {} {} [{}]", method, uri, request_id);
    
    let response = next.run(request).await;
    
    let status = response.status();
    let elapsed = start.elapsed();
    tracing::info!("响应: {} {:.2}s [{}]", status, elapsed.as_secs_f64(), request_id);
    
    response
}
