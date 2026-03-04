use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

// HTTP 请求/响应模型

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub result: String,
    pub query: String,
    pub duration_ms: u64,
    pub model: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_queries: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub request_id: String,
}

// SSE 事件数据结构

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SseEventData {
    Start {
        query: String,
    },
    Split {
        sub_queries: Vec<String>,
    },
    Progress {
        index: usize,
        query: String,
        status: String,
    },
    Result {
        index: usize,
        content: String,
    },
    Complete {
        total: usize,
        duration_ms: u64,
        model: String,
        timestamp: String,
    },
    Error {
        message: String,
        code: String,
    },
    Ping {
        timestamp: String,
    },
}

// MCP 协议数据结构

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

// MCP 会话管理
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct McpSession {
    pub tx: mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
    pub last_activity: std::sync::Arc<std::sync::Mutex<std::time::Instant>>,
}

pub struct McpSessionManager {
    sessions: Mutex<HashMap<String, McpSession>>,
}

impl Default for McpSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
    
    pub fn add_session(&self, session_id: String, tx: mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>) {
        let now = std::time::Instant::now();
        let session = McpSession {
            tx,
            last_activity: std::sync::Arc::new(std::sync::Mutex::new(now)),
        };
        
        self.sessions.lock().unwrap().insert(session_id, session);
    }
    
    pub fn get_session(&self, session_id: &str) -> Option<mpsc::Sender<Result<axum::response::sse::Event, std::convert::Infallible>>> {
        let sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get(session_id) {
            // 更新最后活动时间
            *session.last_activity.lock().unwrap() = std::time::Instant::now();
            Some(session.tx.clone())
        } else {
            None
        }
    }
    
    pub fn remove_session(&self, session_id: &str) {
        self.sessions.lock().unwrap().remove(session_id);
    }
    
    pub fn cleanup_expired(&self, timeout_secs: u64) -> usize {
        let mut sessions = self.sessions.lock().unwrap();
        let now = std::time::Instant::now();
        let mut removed_count = 0;
        
        sessions.retain(|session_id, session| {
            let last_activity = *session.last_activity.lock().unwrap();
            let elapsed = now.duration_since(last_activity).as_secs();
            if elapsed > timeout_secs {
                tracing::info!("清理过期会话: {} (最后活动: {} 秒前)", session_id, elapsed);
                removed_count += 1;
                false
            } else {
                true
            }
        });
        
        removed_count
    }
}

// 应用状态

pub struct AppState {
    pub config: Arc<tokio::sync::RwLock<crate::config::AIConfig>>,
    pub ai_client: Arc<tokio::sync::RwLock<crate::client::AIClient>>,
    pub sse_pool: Arc<crate::sse::SseConnectionPool>,
    pub mcp_sessions: Arc<McpSessionManager>,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn new(config: Arc<crate::config::AIConfig>, ai_client: Arc<crate::client::AIClient>) -> Self {
        let sse_pool = Arc::new(crate::sse::SseConnectionPool::new(config.http_max_sse_connections));
        let mcp_sessions = Arc::new(McpSessionManager::new());
        
        // 启动全局会话清理任务
        let sessions_for_cleanup = mcp_sessions.clone();
        let session_timeout = config.http_session_timeout;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(session_timeout / 3)).await;
                let removed = sessions_for_cleanup.cleanup_expired(session_timeout);
                if removed > 0 {
                    tracing::info!("全局清理: 清理了 {} 个过期会话", removed);
                }
            }
        });
        
        Self {
            config: Arc::new(tokio::sync::RwLock::new((*config).clone())),
            ai_client: Arc::new(tokio::sync::RwLock::new((*ai_client).clone())),
            sse_pool,
            mcp_sessions,
            start_time: std::time::Instant::now(),
        }
    }
    
    pub async fn reload_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config_file::ConfigFile;
        
        // 从文件重新加载配置
        let config_file = ConfigFile::load()?;
        let new_config = crate::config::AIConfig::from(config_file);
        
        // 创建新的 AIClient
        let new_client = crate::client::AIClient::new(new_config.clone())?;
        
        // 更新配置和客户端
        *self.config.write().await = new_config;
        *self.ai_client.write().await = new_client;
        
        tracing::info!("配置已成功重新加载");
        Ok(())
    }
}

// 错误类型

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppError {
    InvalidRequest(String),
    Unauthorized(String),
    SearchError(String),
    SseConnectionLimit,
    PayloadTooLarge,
    InternalError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg),
            AppError::SearchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "SEARCH_ERROR", msg),
            AppError::SseConnectionLimit => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SSE_CONNECTION_LIMIT",
                "Maximum SSE connections reached".to_string(),
            ),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Request body exceeds size limit".to_string(),
            ),
            AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg),
        };
        
        let request_id = Uuid::new_v4().to_string();
        
        let body = Json(ErrorResponse {
            error: message,
            code: code.to_string(),
            request_id,
        });
        
        (status, body).into_response()
    }
}
