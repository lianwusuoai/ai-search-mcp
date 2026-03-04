use crate::error::{AISearchError, Result};
use crate::config_file::ConfigFile;
use serde::{Deserialize, Serialize};
use std::env;

const DEFAULT_TIMEOUT: u64 = 60;
const DEFAULT_RETRY_COUNT: u32 = 1;
const DEFAULT_MAX_QUERY_PLAN: u32 = 1;

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是一个专业的搜索助手,擅长联网搜索并提供准确、详细的答案。

当前时间: {current_time}

搜索策略:
1. 优先使用最新、权威的信息源
2. 对于时间敏感的查询,明确标注信息的时间
3. 提供多个来源的信息进行交叉验证
4. 对于技术问题,优先参考官方文档和最新版本

输出要求:
- 直接回答用户问题
- 时间相关信息必须基于上述当前时间判断"#;

pub const DEFAULT_SPLIT_PROMPT: &str = "你是查询拆分助手。只返回 JSON 数组，不要任何解释、标记或其他文本。直接输出 JSON 数组。";

#[derive(Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub api_url: String,
    pub api_key: String,
    pub search_model_id: String,
    pub analysis_model_id: Option<String>,
    pub system_prompt: String,
    pub split_prompt: String,
    pub timeout: u64,
    pub stream: bool,
    pub filter_thinking: bool,
    pub retry_count: u32,
    pub analysis_retry_count: u32,
    pub search_retry_count: u32,
    pub log_level: String,
    pub max_query_plan: u32,
    // HTTP/SSE 相关配置
    pub http_api_key: String,
    pub http_sse_heartbeat: u64,
    pub http_max_sse_connections: usize,
    pub http_max_body_size: usize,
    pub http_mcp_channel_capacity: usize,
    pub http_session_timeout: u64,
}

impl std::fmt::Debug for AIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIConfig")
            .field("api_url", &self.api_url)
            .field("api_key", &"***REDACTED***")
            .field("search_model_id", &self.search_model_id)
            .field("analysis_model_id", &self.analysis_model_id)
            .field("system_prompt", &"<omitted>")
            .field("split_prompt", &"<omitted>")
            .field("timeout", &self.timeout)
            .field("stream", &self.stream)
            .field("filter_thinking", &self.filter_thinking)
            .field("retry_count", &self.retry_count)
            .field("analysis_retry_count", &self.analysis_retry_count)
            .field("search_retry_count", &self.search_retry_count)
            .field("log_level", &self.log_level)
            .field("max_query_plan", &self.max_query_plan)
            .field("http_api_key", &"***REDACTED***")
            .field("http_sse_heartbeat", &self.http_sse_heartbeat)
            .field("http_max_sse_connections", &self.http_max_sse_connections)
            .field("http_max_body_size", &self.http_max_body_size)
            .field("http_mcp_channel_capacity", &self.http_mcp_channel_capacity)
            .field("http_session_timeout", &self.http_session_timeout)
            .finish()
    }
}

impl From<ConfigFile> for AIConfig {
    fn from(config_file: ConfigFile) -> Self {
        Self {
            api_url: config_file.api_url,
            api_key: config_file.api_key,
            search_model_id: config_file.search_model_id,
            analysis_model_id: config_file.analysis_model_id,
            system_prompt: config_file.system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()),
            split_prompt: config_file.split_prompt.unwrap_or_else(|| DEFAULT_SPLIT_PROMPT.to_string()),
            timeout: config_file.timeout,
            stream: config_file.stream,
            filter_thinking: config_file.filter_thinking,
            retry_count: config_file.retry_count,
            analysis_retry_count: config_file.analysis_retry_count,
            search_retry_count: config_file.search_retry_count,
            log_level: config_file.log_level,
            max_query_plan: config_file.max_query_plan,
            http_api_key: config_file.http_api_key,
            // HTTP/SSE 默认值（心跳间隔 5 秒，针对移动网络优化）
            http_sse_heartbeat: 5,
            http_max_sse_connections: 100,
            http_max_body_size: 10 * 1024 * 1024,
            http_mcp_channel_capacity: 100,
            http_session_timeout: 1800,
        }
    }
}

impl AIConfig {
    pub fn from_env() -> Result<Self> {
        // 优先尝试从配置文件加载
        if let Ok(config_file) = ConfigFile::load() {
            tracing::info!("从配置文件加载配置: {:?}", ConfigFile::config_path().ok());
            return Ok(config_file.into());
        }
        
        tracing::info!("配置文件不存在，从环境变量加载配置");
        
        // 配置文件不存在,从环境变量加载
        let api_url = env::var("AI_API_URL")
            .map_err(|_| AISearchError::Config("缺少 AI_API_URL 环境变量或配置文件".into()))?;
        
        let api_key = env::var("AI_API_KEY")
            .map_err(|_| AISearchError::Config("缺少 AI_API_KEY 环境变量".into()))?;
        
        let search_model_id = env::var("AI_SEARCH_MODEL_ID")
            .or_else(|_| env::var("AI_MODEL_ID"))
            .map_err(|_| AISearchError::Config("缺少 AI_SEARCH_MODEL_ID 或 AI_MODEL_ID 环境变量".into()))?;
        
        let analysis_model_id = env::var("AI_ANALYSIS_MODEL_ID").ok();
        
        let system_prompt = env::var("AI_SYSTEM_PROMPT")
            .unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string());
        
        let split_prompt = env::var("AI_SPLIT_PROMPT")
            .unwrap_or_else(|_| DEFAULT_SPLIT_PROMPT.to_string());
        
        let timeout = env::var("AI_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT);
        
        let stream = env::var("AI_STREAM")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(true);
        
        let filter_thinking = env::var("AI_FILTER_THINKING")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(true);
        
        let retry_count = env::var("AI_RETRY_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RETRY_COUNT);
        
        let analysis_retry_count = env::var("AI_ANALYSIS_RETRY_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        
        let search_retry_count = env::var("AI_SEARCH_RETRY_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        
        let log_level = env::var("AI_LOG_LEVEL")
            .unwrap_or_else(|_| "INFO".to_string())
            .to_uppercase();
        
        let max_query_plan = env::var("AI_MAX_QUERY_PLAN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_QUERY_PLAN);
        
        // HTTP/SSE 相关配置
        let mut http_api_key = env::var("AI_HTTP_API_KEY")
            .unwrap_or_else(|_| "xinchen".to_string());
        
        // 验证 API Key
        if http_api_key.is_empty() {
            return Err(AISearchError::Config("API key 不能为空".into()));
        }
        
        if http_api_key.len() > 256 {
            tracing::warn!("API key 长度超过 256 字符，将截断");
            http_api_key = http_api_key[..256].to_string();
        }
        
        if http_api_key == "xinchen" {
            tracing::warn!("使用默认 API key 'xinchen'。生产环境请设置 AI_HTTP_API_KEY");
        }
        
        let http_sse_heartbeat = env::var("AI_HTTP_SSE_HEARTBEAT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5); // 默认 5 秒，针对移动网络优化
        
        let http_max_sse_connections = env::var("AI_HTTP_MAX_SSE_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        
        let http_max_body_size_mb = env::var("AI_HTTP_MAX_BODY_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);
        let http_max_body_size = http_max_body_size_mb * 1024 * 1024;
        
        let http_mcp_channel_capacity = env::var("AI_HTTP_MCP_CHANNEL_CAPACITY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        
        let http_session_timeout = env::var("AI_HTTP_SESSION_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1800); // 默认 30 分钟
        
        let config = Self {
            api_url,
            api_key,
            search_model_id,
            analysis_model_id,
            system_prompt,
            split_prompt,
            timeout,
            stream,
            filter_thinking,
            retry_count,
            analysis_retry_count,
            search_retry_count,
            log_level,
            max_query_plan,
            http_api_key,
            http_sse_heartbeat,
            http_max_sse_connections,
            http_max_body_size,
            http_mcp_channel_capacity,
            http_session_timeout,
        };
        
        config.validate()?;
        Ok(config)
    }
    
    fn validate(&self) -> Result<()> {
        if !self.api_url.starts_with("http://") && !self.api_url.starts_with("https://") {
            return Err(AISearchError::Config(
                format!("API URL 必须以 http:// 或 https:// 开头: {}", self.api_url)
            ));
        }
        
        if self.timeout < 1 || self.timeout > 300 {
            return Err(AISearchError::Config(
                format!("超时时间必须在 1-300 秒之间: {}", self.timeout)
            ));
        }
        
        if self.max_query_plan < 1 || self.max_query_plan > 1000 {
            return Err(AISearchError::Config(
                format!("最大子查询数必须在 1-1000 之间: {}", self.max_query_plan)
            ));
        }
        
        Ok(())
    }
}
