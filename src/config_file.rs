use crate::error::{AISearchError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub api_url: String,
    pub api_key: String,
    pub search_model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis_model_id: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_stream")]
    pub stream: bool,
    #[serde(default = "default_filter_thinking")]
    pub filter_thinking: bool,
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    #[serde(default = "default_analysis_retry_count")]
    pub analysis_retry_count: u32,
    #[serde(default = "default_search_retry_count")]
    pub search_retry_count: u32,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_max_query_plan")]
    pub max_query_plan: u32,
    #[serde(default = "default_http_api_key")]
    pub http_api_key: String,
    #[serde(default = "default_admin_password")]
    pub admin_password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_prompt: Option<String>,
}

fn default_timeout() -> u64 { 180 }
fn default_stream() -> bool { true }
fn default_filter_thinking() -> bool { true }
fn default_retry_count() -> u32 { 1 }
fn default_analysis_retry_count() -> u32 { 1 }
fn default_search_retry_count() -> u32 { 0 }
fn default_log_level() -> String { "INFO".to_string() }
fn default_max_query_plan() -> u32 { 10 }
fn default_http_api_key() -> String { "xinchen".to_string() }
fn default_admin_password() -> String { "xinchen".to_string() }

impl ConfigFile {
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| AISearchError::Config("无法获取用户主目录".into()))?;
        Ok(home.join(".ai-search-mcp"))
    }
    
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }
    
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        
        if !path.exists() {
            return Err(AISearchError::Config(
                format!("配置文件不存在: {:?}", path)
            ));
        }
        
        let content = fs::read_to_string(&path)
            .map_err(|e| AISearchError::Config(format!("读取配置文件失败: {}", e)))?;
        
        let config: Self = serde_json::from_str(&content)
            .map_err(|e| AISearchError::Config(format!("解析配置文件失败: {}", e)))?;
        
        config.validate()?;
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)
            .map_err(|e| AISearchError::Config(format!("创建配置目录失败: {}", e)))?;
        
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| AISearchError::Config(format!("序列化配置失败: {}", e)))?;
        
        fs::write(&path, content)
            .map_err(|e| AISearchError::Config(format!("写入配置文件失败: {}", e)))?;
        
        tracing::info!("配置已保存到: {:?}", path);
        Ok(())
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.api_url.is_empty() {
            return Err(AISearchError::Config("API URL 不能为空".into()));
        }
        
        if !self.api_url.starts_with("http://") && !self.api_url.starts_with("https://") {
            return Err(AISearchError::Config("API URL 必须以 http:// 或 https:// 开头".into()));
        }
        
        if self.api_key.is_empty() {
            return Err(AISearchError::Config("API Key 不能为空".into()));
        }
        
        if self.search_model_id.is_empty() {
            return Err(AISearchError::Config("搜索模型 ID 不能为空".into()));
        }
        
        if self.timeout < 1 || self.timeout > 300 {
            return Err(AISearchError::Config("超时时间必须在 1-300 秒之间".into()));
        }
        
        Ok(())
    }
}
