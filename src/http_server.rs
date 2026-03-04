use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::limit::RequestBodyLimitLayer;

use crate::config::AIConfig;
use crate::client::AIClient;
use crate::handlers::{health_handler, search_handler, search_stream_handler};
use crate::mcp_sse::mcp_sse_handler;
use crate::mcp_handler::{mcp_handler, mcp_http_handler};
use crate::middleware::{auth_middleware, cors_layer, logging_middleware};
use crate::models::AppState;
use crate::config_ui;

pub struct HttpServer {
    config: Arc<AIConfig>,
    ai_client: Arc<AIClient>,
}

impl HttpServer {
    pub fn new(config: Arc<AIConfig>, ai_client: Arc<AIClient>) -> Self {
        Self { config, ai_client }
    }
    
    pub async fn run(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let app = self.build_router();
        
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("HTTP 服务器监听: http://{}", addr);
        tracing::info!("配置界面: http://localhost:{}", port);
        tracing::info!("请求体大小限制: {} MB", self.config.http_max_body_size / 1024 / 1024);
        
        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        
        tracing::info!("服务器已停止");
        Ok(())
    }
    
    fn build_router(&self) -> Router {
        let state = Arc::new(AppState::new(self.config.clone(), self.ai_client.clone()));
        
        // 配置界面路由（无需认证）
        let config_router = config_ui::create_router(state.clone());
        
        // 公开路由（无需认证）
        let public_router = Router::new()
            .route("/health", get(health_handler))
            .with_state(state.clone());
        
        // MCP 和 API 路由（需要认证）
        let protected_router = Router::new()
            .route("/sse", get(mcp_sse_handler))
            .route("/mcp", post(mcp_handler))  // SSE 模式（需要 session）
            .route("/http", post(mcp_http_handler))  // HTTP 模式（简化路径）
            .route("/api/search", post(search_handler))
            .route("/api/search/stream", post(search_stream_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);
        
        // 合并路由：根路径和 /config 都指向配置界面，不经过认证中间件
        Router::new()
            .merge(config_router)  // 根路径 / 显示配置界面
            .merge(public_router)  // /health 无需认证
            .merge(protected_router)  // 其他端点需要认证
            .layer(middleware::from_fn(logging_middleware))
            .layer(cors_layer())
            .layer(RequestBodyLimitLayer::new(self.config.http_max_body_size))
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("无法安装 Ctrl+C 处理器");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("无法安装 SIGTERM 处理器")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("收到 SIGINT (Ctrl+C)，优雅关闭中...");
        },
        _ = terminate => {
            tracing::info!("收到 SIGTERM，优雅关闭中...");
        },
    }
}
