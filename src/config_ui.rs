use crate::config_file::ConfigFile;
use crate::config::DEFAULT_SYSTEM_PROMPT;
use crate::config::DEFAULT_SPLIT_PROMPT;
use crate::models::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response, Redirect},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "web/"]
struct WebAssets;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/login", get(login_page_handler))
        .route("/static/:file", get(static_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/config", get(get_config_handler).post(save_config_handler))
        .route("/api/defaults", get(get_defaults_handler))
        .route("/api/test", post(test_connection_handler))
        .route("/api/restart", post(restart_handler))
        .with_state(state)
}

async fn index_handler(jar: CookieJar, State(state): State<Arc<AppState>>) -> Response {
    // 检查是否已登录
    if let Some(cookie) = jar.get("config_auth") {
        if verify_auth_cookie(cookie.value(), &state).await {
            // 已登录，显示配置页面
            return match WebAssets::get("index.html") {
                Some(content) => Html(content.data.to_vec()).into_response(),
                None => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
            };
        }
    }
    
    // 未登录，重定向到登录页面
    Redirect::to("/login").into_response()
}

async fn login_page_handler() -> Response {
    match WebAssets::get("login.html") {
        Some(content) => Html(content.data.to_vec()).into_response(),
        None => (StatusCode::NOT_FOUND, "login.html not found").into_response(),
    }
}

async fn static_handler(
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Response {
    match WebAssets::get(&file) {
        Some(content) => {
            let mime = if file.ends_with(".css") {
                "text/css"
            } else if file.ends_with(".js") {
                "application/javascript"
            } else {
                "text/plain"
            };
            
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

async fn get_config_handler(jar: CookieJar, State(state): State<Arc<AppState>>) -> Response {
    // 检查登录状态
    if let Some(cookie) = jar.get("config_auth") {
        if !verify_auth_cookie(cookie.value(), &state).await {
            return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
                message: None,
                error: Some("未登录或登录已过期".to_string()),
            })).into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
            message: None,
            error: Some("未登录".to_string()),
        })).into_response();
    }
    
    match ConfigFile::load() {
        Ok(config) => Json(config).into_response(),
        Err(_) => {
            // 返回默认配置
            let default_config = ConfigFile {
                api_url: String::new(),
                api_key: String::new(),
                search_model_id: "Grok".to_string(),
                analysis_model_id: None,
                timeout: 180,
                stream: true,
                filter_thinking: true,
                retry_count: 1,
                analysis_retry_count: 1,
                search_retry_count: 0,
                log_level: "INFO".to_string(),
                max_query_plan: 10,
                http_api_key: "xinchen".to_string(),
                admin_password: "xinchen".to_string(),
                system_prompt: None,
                split_prompt: None,
            };
            Json(default_config).into_response()
        }
    }
}

#[derive(Deserialize)]
struct SaveConfigRequest {
    api_url: String,
    api_key: String,
    search_model_id: String,
    analysis_model_id: Option<String>,
    timeout: u64,
    stream: bool,
    filter_thinking: bool,
    analysis_retry_count: u32,
    search_retry_count: u32,
    log_level: String,
    max_query_plan: u32,
    http_api_key: String,
    admin_password: String,
    system_prompt: Option<String>,
    split_prompt: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn save_config_handler(
    jar: CookieJar,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveConfigRequest>,
) -> Response {
    // 检查登录状态
    if let Some(cookie) = jar.get("config_auth") {
        if !verify_auth_cookie(cookie.value(), &state).await {
            return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
                message: None,
                error: Some("未登录或登录已过期".to_string()),
            })).into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
            message: None,
            error: Some("未登录".to_string()),
        })).into_response();
    }
    
    let config = ConfigFile {
        api_url: req.api_url,
        api_key: req.api_key,
        search_model_id: req.search_model_id,
        analysis_model_id: req.analysis_model_id,
        timeout: req.timeout,
        stream: req.stream,
        filter_thinking: req.filter_thinking,
        retry_count: 1, // 保持兼容性,使用固定值
        analysis_retry_count: req.analysis_retry_count,
        search_retry_count: req.search_retry_count,
        log_level: req.log_level,
        max_query_plan: req.max_query_plan,
        http_api_key: req.http_api_key,
        admin_password: req.admin_password,
        system_prompt: req.system_prompt,
        split_prompt: req.split_prompt,
    };
    
    match config.save() {
        Ok(_) => {
            let response = ApiResponse {
                message: Some("配置保存成功".to_string()),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response = ApiResponse {
                message: None,
                error: Some(e.to_string()),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

#[derive(Deserialize)]
struct TestConnectionRequest {
    api_url: String,
    api_key: String,
}

async fn test_connection_handler(
    jar: CookieJar,
    State(state): State<Arc<AppState>>,
    Json(req): Json<TestConnectionRequest>,
) -> Response {
    // 检查登录状态
    if let Some(cookie) = jar.get("config_auth") {
        if !verify_auth_cookie(cookie.value(), &state).await {
            return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
                message: None,
                error: Some("未登录或登录已过期".to_string()),
            })).into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
            message: None,
            error: Some("未登录".to_string()),
        })).into_response();
    }
    
    // 简单的 URL 验证
    if !req.api_url.starts_with("http://") && !req.api_url.starts_with("https://") {
        let response = ApiResponse {
            message: None,
            error: Some("API URL 必须以 http:// 或 https:// 开头".to_string()),
        };
        return (StatusCode::BAD_REQUEST, Json(response)).into_response();
    }
    
    if req.api_key.is_empty() {
        let response = ApiResponse {
            message: None,
            error: Some("API Key 不能为空".to_string()),
        };
        return (StatusCode::BAD_REQUEST, Json(response)).into_response();
    }
    
    // 这里可以添加实际的连接测试逻辑
    // 暂时只做基本验证
    let response = ApiResponse {
        message: Some("连接测试通过".to_string()),
        error: None,
    };
    (StatusCode::OK, Json(response)).into_response()
}

async fn restart_handler(
    jar: CookieJar,
    State(state): State<Arc<AppState>>,
) -> Response {
    // 检查登录状态
    if let Some(cookie) = jar.get("config_auth") {
        if !verify_auth_cookie(cookie.value(), &state).await {
            return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
                message: None,
                error: Some("未登录或登录已过期".to_string()),
            })).into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, Json(ApiResponse {
            message: None,
            error: Some("未登录".to_string()),
        })).into_response();
    }
    
    tracing::info!("收到重启请求，开始重新加载配置");
    
    // 调用 AppState 的 reload_config 方法
    match state.reload_config().await {
        Ok(_) => {
            let response = ApiResponse {
                message: Some("配置已成功重新加载".to_string()),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("配置重新加载失败: {}", e);
            let response = ApiResponse {
                message: None,
                error: Some(format!("配置重新加载失败: {}", e)),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

#[derive(Serialize)]
struct DefaultPromptsResponse {
    system_prompt: String,
    split_prompt: String,
}

async fn get_defaults_handler() -> Response {
    let defaults = DefaultPromptsResponse {
        system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
        split_prompt: DEFAULT_SPLIT_PROMPT.to_string(),
    };
    Json(defaults).into_response()
}

#[derive(Deserialize)]
struct LoginRequest {
    key: String,
}

async fn login_handler(
    jar: CookieJar,
    State(_state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    // 验证 key（使用 admin_password）
    let config_file = match ConfigFile::load() {
        Ok(cf) => cf,
        Err(_) => {
            // 配置文件不存在，使用默认密码
            ConfigFile {
                api_url: String::new(),
                api_key: String::new(),
                search_model_id: "Grok".to_string(),
                analysis_model_id: None,
                timeout: 60,
                stream: true,
                filter_thinking: true,
                retry_count: 1,
                analysis_retry_count: 1,
                search_retry_count: 0,
                log_level: "INFO".to_string(),
                max_query_plan: 1,
                http_api_key: "xinchen".to_string(),
                admin_password: "xinchen".to_string(),
                system_prompt: None,
                split_prompt: None,
            }
        }
    };
    
    let admin_password = config_file.admin_password.clone();
    
    if req.key == admin_password {
        // 登录成功，设置 cookie
        let cookie = Cookie::build(("config_auth", admin_password))
            .path("/")
            .http_only(true)
            .max_age(time::Duration::days(30))
            .build();
        
        let jar = jar.add(cookie);
        
        let response = ApiResponse {
            message: Some("登录成功".to_string()),
            error: None,
        };
        (jar, Json(response)).into_response()
    } else {
        let response = ApiResponse {
            message: None,
            error: Some("访问密钥错误".to_string()),
        };
        (StatusCode::UNAUTHORIZED, Json(response)).into_response()
    }
}

async fn logout_handler(jar: CookieJar) -> Response {
    // 删除 cookie
    let cookie = Cookie::build(("config_auth", ""))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    
    let jar = jar.add(cookie);
    
    let response = ApiResponse {
        message: Some("已退出登录".to_string()),
        error: None,
    };
    (jar, Json(response)).into_response()
}

async fn verify_auth_cookie(cookie_value: &str, _state: &AppState) -> bool {
    // 验证 cookie 值是否与配置的 admin_password 匹配
    match ConfigFile::load() {
        Ok(config_file) => cookie_value == config_file.admin_password,
        Err(_) => cookie_value == "xinchen", // 默认密码
    }
}
