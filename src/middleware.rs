use axum::{
    extract::State,
    http::{Request, StatusCode, Method, HeaderMap},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

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
    
    // /http 端点需要 API key 认证（支持 query 参数和 Authorization header）
    if path == "/http" {
        let config = state.config.read().await;
        let expected_key = &config.http_api_key;
        
        // 方式 1: 检查 Authorization header（标准方式）
        if let Some(auth_header) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
            let api_key = if auth_header.starts_with("Bearer ") {
                auth_header.strip_prefix("Bearer ").unwrap()
            } else {
                auth_header
            };
            
            if api_key == expected_key {
                tracing::debug!("MCP HTTP 认证成功 (Authorization header)");
                return Ok(next.run(request).await);
            } else {
                tracing::warn!("MCP HTTP 认证失败: Authorization header 中的 API key 不匹配");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        
        // 方式 2: 检查 query 参数（兼容 Cherry Studio 等客户端）
        let query = request.uri().query().unwrap_or("");
        if let Some(key_value) = query.split('&')
            .find(|param| param.starts_with("key="))
            .and_then(|param| param.strip_prefix("key=")) {
            if key_value == expected_key {
                tracing::debug!("MCP HTTP 认证成功 (query 参数)");
                return Ok(next.run(request).await);
            } else {
                tracing::warn!("MCP HTTP 认证失败: query 参数中的 API key 不匹配");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        
        tracing::warn!("MCP HTTP 认证失败: 缺少 Authorization header 或 key 参数");
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 配置界面相关端点使用 cookie 认证（在 config_ui.rs 中处理）
    // 不需要 API key 认证
    if path == "/" 
        || path == "/login" 
        || path == "/favicon.ico" 
        || path.starts_with("/static/") 
        || path.starts_with("/api/auth/") 
        || path.starts_with("/api/config") 
        || path.starts_with("/api/defaults") 
        || path.starts_with("/api/test") 
        || path.starts_with("/api/restart")
        || path.starts_with("/.well-known/") {  // OAuth 发现端点
        return Ok(next.run(request).await);
    }
    
    // /sse 端点需要 API key 认证（支持 query 参数和 Authorization header）
    if path == "/sse" {
        let config = state.config.read().await;
        let expected_key = &config.http_api_key;
        
        // 方式 1: 检查 Authorization header（标准方式，Cherry Studio 等客户端优先使用）
        if let Some(auth_header) = headers.get("Authorization").and_then(|v| v.to_str().ok()) {
            let api_key = if auth_header.starts_with("Bearer ") {
                auth_header.strip_prefix("Bearer ").unwrap()
            } else {
                auth_header
            };
            
            if api_key == expected_key {
                tracing::debug!("SSE 认证成功 (Authorization header)");
                return Ok(next.run(request).await);
            } else {
                tracing::warn!("SSE 认证失败: Authorization header 中的 API key 不匹配");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        
        // 方式 2: 检查 query 参数（兼容旧版本和特殊客户端）
        let query = request.uri().query().unwrap_or("");
        if let Some(key_value) = query.split('&')
            .find(|param| param.starts_with("key="))
            .and_then(|param| param.strip_prefix("key=")) {
            if key_value == expected_key {
                tracing::debug!("SSE 认证成功 (query 参数)");
                return Ok(next.run(request).await);
            } else {
                tracing::warn!("SSE 认证失败: query 参数中的 API key 不匹配");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        
        tracing::warn!("SSE 认证失败: 缺少 Authorization header 或 key 参数");
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
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let start = std::time::Instant::now();
    
    // 降级为 DEBUG 的路径（避免日志污染）
    let is_quiet_path = path == "/health" 
        || (path == "/api/config" && method == Method::GET)
        || path == "/mcp"
        || path == "/favicon.ico"
        || path == "/sse"
        || path == "/"
        || path.starts_with("/static/")
        || path == "/api/defaults"
        || path == "/api/search";  // 搜索请求降为 DEBUG
    
    if is_quiet_path {
        tracing::debug!("请求: {} {}", method, uri);
    } else {
        tracing::info!("请求: {} {}", method, uri);
    }
    
    let response = next.run(request).await;
    
    let status = response.status();
    let elapsed = start.elapsed();
    
    if is_quiet_path {
        tracing::debug!("响应: {} {:.2}s", status, elapsed.as_secs_f64());
    } else {
        tracing::info!("响应: {} {:.2}s", status, elapsed.as_secs_f64());
    }
    
    response
}
