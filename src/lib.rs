// 公共 API 导出，供测试和外部使用

pub mod error;
pub mod error_codes;
pub mod config_file;
pub mod config;
pub mod config_ui;
pub mod client;
pub mod stdio_handler;
pub mod models;
pub mod sse;
pub mod middleware;
pub mod handlers;
pub mod http_server;
pub mod mcp_sse;
pub mod mcp_handler;
pub mod daemon;

// 重新导出常用类型
pub use client::AIClient;
pub use config::AIConfig;
pub use error::{AISearchError, Result};
pub use stdio_handler::MCPServer;
