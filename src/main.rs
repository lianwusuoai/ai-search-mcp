mod config;
mod config_file;
mod config_ui;
mod client;
mod error;
mod error_codes;
mod stdio_handler;
mod models;
mod sse;
mod middleware;
mod handlers;
mod http_server;
mod daemon;
mod mcp_sse;
mod mcp_handler;

use config::AIConfig;
use error::Result;
use stdio_handler::MCPServer;
use http_server::HttpServer;
use daemon::DaemonManager;
use clap::Parser;
use std::sync::Arc;
use std::fmt;
use tracing_subscriber::fmt::time::FormatTime;

// 自定义时间格式化器：北京时间（东八区）+ 2位小数精度
struct LocalTimeFormatter;

impl FormatTime for LocalTimeFormatter {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> fmt::Result {
        // 使用 UTC 时间 + 8 小时偏移（东八区）
        let now = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        // 格式化为自定义格式（日期和时间之间两个空格），手动截取毫秒到 2 位
        let timestamp = now.format("%Y-%m-%d  %H:%M:%S%.3f").to_string();
        // 截取到 2 位小数：2026-03-04  03:28:09.123 -> 2026-03-04  03:28:09.12
        let truncated = if let Some(dot_pos) = timestamp.rfind('.') {
            format!("{}.{}", &timestamp[..dot_pos], &timestamp[dot_pos+1..dot_pos+3])
        } else {
            timestamp
        };
        write!(w, "{}", truncated)
    }
}

#[derive(Parser, Debug)]
#[command(name = "ai-search-mcp-server")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "AI Search MCP Server with HTTP/SSE support")]
struct Cli {
    /// 运行模式：stdio（默认）或 http
    #[arg(long, default_value = "stdio")]
    mode: String,
    
    /// HTTP 服务器端口（仅 http 模式，默认: 11000）
    #[arg(long, default_value = "11000")]
    port: u16,
    
    /// 验证配置并退出
    #[arg(long)]
    validate_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 tracing - 使用本地时区和简化格式
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_target(false)  // 禁用模块路径
        .with_timer(LocalTimeFormatter)  // 使用自定义时间格式化器
        .init();
    
    let cli = Cli::parse();
    
    // 加载配置
    let config = Arc::new(AIConfig::from_env()?);
    
    // 验证配置
    if cli.validate_config {
        tracing::info!("配置验证通过");
        tracing::info!("API URL: {}", config.api_url);
        tracing::info!("模型: {}", config.search_model_id);
        tracing::info!("超时: {}秒", config.timeout);
        tracing::info!("HTTP API Key: ***");
        tracing::info!("SSE 心跳间隔: {}秒", config.http_sse_heartbeat);
        tracing::info!("最大 SSE 连接数: {}", config.http_max_sse_connections);
        tracing::info!("最大请求体大小: {} MB", config.http_max_body_size / 1024 / 1024);
        tracing::info!("会话超时: {}秒", config.http_session_timeout);
        return Ok(());
    }
    
    // 初始化 AIClient
    let ai_client = Arc::new(client::AIClient::new((*config).clone())?);
    
    // 选择运行模式
    match cli.mode.as_str() {
        "http" => {
            // HTTP 模式 - 守护进程管理
            let mut daemon = DaemonManager::new(cli.port)
                .map_err(|e| error::AISearchError::Config(format!("守护进程初始化失败: {}", e)))?;
            
            // 原子化检查并获取端口
            match daemon.try_acquire_port() {
                Ok(false) => {
                    tracing::warn!("检测到 HTTP 服务器已在端口 {} 运行,直接退出", cli.port);
                    return Ok(());
                }
                Err(e) => {
                    return Err(error::AISearchError::Config(e.to_string()));
                }
                Ok(true) => {
                    // 端口获取成功,继续启动
                }
            }
            
            // 创建 PID 文件
            daemon.create_pid_file()
                .map_err(|e| error::AISearchError::Config(format!("创建 PID 文件失败: {}", e)))?;
            
            tracing::info!("启动 HTTP 模式");
            tracing::info!("API URL: {}", config.api_url);
            tracing::info!("模型: {}", config.search_model_id);
            
            let server = HttpServer::new(config, ai_client);
            let result = server.run(cli.port).await.map_err(|e| {
                if e.to_string().contains("Address already in use") {
                    error::AISearchError::Config(format!("错误: 端口 {} 已被占用", cli.port))
                } else {
                    error::AISearchError::Config(e.to_string())
                }
            });
            
            // 确保清理 PID 文件
            drop(daemon);
            
            result?;
            Ok(())
        }
        "stdio" => {
            // stdio 模式（默认）
            tracing::info!("启动 stdio 模式");
            let server = MCPServer::new((*config).clone())?;
            server.run().await
        }
        _ => {
            Err(error::AISearchError::Config(format!("未知运行模式: {}，支持的模式: stdio, http", cli.mode)))
        }
    }
}
