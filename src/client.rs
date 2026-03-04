use crate::config::AIConfig;
use crate::error::{AISearchError, Result};
use chrono::Local;
use futures::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

static THINKING_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)<think(?:ing)?>.*?</think(?:ing)?>")
        .expect("THINKING_PATTERN regex 编译失败")
});

static WHITESPACE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\n\s*\n")
        .expect("WHITESPACE_PATTERN regex 编译失败")
});

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<Message>,
    delta: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Clone)]
pub struct AIClient {
    config: AIConfig,
    client: Client,
}

impl AIClient {
    pub fn new(config: AIConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()?;
        
        Ok(Self { config, client })
    }
    
    /// 处理 HTTP 错误响应，区分用户错误和服务器错误
    async fn handle_error_response(status_code: u16, response: reqwest::Response) -> AISearchError {
        // 区分用户错误和服务器错误
        let (user_message, should_log_detail) = match status_code {
            // 4xx 客户端错误 - 可以返回详细信息
            400 => ("请求参数错误，请检查查询内容".to_string(), false),
            401 => ("认证失败，请检查 API_KEY 是否正确".to_string(), false),
            403 => ("访问被拒绝，请检查 API_KEY 权限".to_string(), false),
            404 => ("API 端点不存在，请检查 API_URL 配置".to_string(), false),
            408 => ("请求超时，请稍后重试".to_string(), false),
            413 => ("请求体过大，请减少查询内容".to_string(), false),
            429 => ("请求过于频繁，建议稍后重试或切换 API 渠道".to_string(), false),
            // 5xx 服务器错误 - 隐藏详细信息
            500..=599 => ("服务暂时不可用，请稍后重试".to_string(), true),
            // 其他错误
            _ => ("请求失败，请稍后重试".to_string(), true),
        };
        
        // 记录详细错误到日志（仅服务器错误）
        if should_log_detail {
            if let Ok(detail) = response.text().await {
                error!("API 错误 (HTTP {}): {}", status_code, detail);
            }
        }
        
        AISearchError::Api {
            code: status_code,
            message: user_message,
        }
    }
    
    pub async fn search(&self, query: &str) -> Result<String> {
        let is_sub_query = query.starts_with("[SUB_QUERY]");
        
        if is_sub_query {
            let actual_query = query.trim_start_matches("[SUB_QUERY]").trim();
            info!("子查询直接搜索: {}", actual_query);
            return self.call_api(actual_query).await;
        }
        
        if self.config.max_query_plan > 1 {
            info!("多维度搜索: 并发执行 {} 个子查询", self.config.max_query_plan);
            
            // 1. 拆分查询
            let sub_queries = self.split_query(query, self.config.max_query_plan).await?;
            info!("拆分完成: {:?}", sub_queries);
            
            // 2. 并发执行所有子查询
            tracing::debug!("开始并发执行 {} 个子查询", sub_queries.len());
            let start_time = std::time::Instant::now();
            
            // 预先创建所有任务句柄，确保同时启动
            let mut search_futures = Vec::with_capacity(sub_queries.len());
            for (i, sub_query) in sub_queries.iter().enumerate() {
                let query = sub_query.clone();
                let client = self.clone();
                let task = tokio::spawn(async move {
                    let result = client.search_internal(&query).await;
                    result
                });
                tracing::debug!("已启动子查询 {}", i + 1);
                search_futures.push(task);
            }
            
            let results: Vec<Result<String>> = futures::future::join_all(search_futures)
                .await
                .into_iter()
                .map(|r| r.unwrap_or_else(|e| Err(AISearchError::Network {
                    message: format!("任务执行失败: {}", e),
                    suggestion: "请重试".into(),
                })))
                .collect();
            let elapsed = start_time.elapsed();
            
            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let fail_count = results.iter().filter(|r| r.is_err()).count();
            info!("并发执行完成: 成功 {}, 失败 {}, 总耗时 {:.2}s", success_count, fail_count, elapsed.as_secs_f64());
            
            // 3. 直接返回所有结果（不整合）
            let mut output = String::new();
            for (i, result) in results.into_iter().enumerate() {
                // 提取子问题（去掉 [SUB_QUERY] 前缀）
                let sub_question = sub_queries.get(i)
                    .map(|q| q.trim_start_matches("[SUB_QUERY]").trim())
                    .unwrap_or("未知");
                
                match result {
                    Ok(content) => {
                        output.push_str(&format!("## 子查询 {} 结果\n\n**子问题**: {}\n\n{}\n\n", i + 1, sub_question, content));
                    }
                    Err(e) => {
                        error!("子查询 {} 失败 (查询: {}): {}", i + 1, sub_question, e);
                        output.push_str(&format!("## 子查询 {} 失败\n\n**子问题**: {}\n\n**错误**: {}\n\n", i + 1, sub_question, e));
                    }
                }
            }
            
            if output.is_empty() {
                return Err(AISearchError::Protocol("所有子查询都失败了".into()));
            }
            
            return Ok(output);
        }
        
        info!("直接搜索: {}", query);
        self.call_api(query).await
    }
    
    /// 内部搜索方法，用于递归调用
    async fn search_internal(&self, query: &str) -> Result<String> {
        let is_sub_query = query.starts_with("[SUB_QUERY]");
        
        if is_sub_query {
            let actual_query = query.trim_start_matches("[SUB_QUERY]").trim();
            info!("子查询直接搜索: {}", actual_query);
            return self.call_api(actual_query).await;
        }
        
        info!("直接搜索: {}", query);
        self.call_api(query).await
    }
    
    /// 通用 API 调用方法,支持自定义模型和提示词
    async fn call_api_internal(
        &self, 
        query: &str, 
        custom_prompt: Option<&str>, 
        model_id: Option<&str>,
        retry_count: u32
    ) -> Result<String> {
        let retryable_codes = [401, 402, 403, 408, 429, 500, 501, 502, 503, 504];
        let mut last_error = None;
        
        for attempt in 0..=retry_count {
            let result = if let (Some(prompt), Some(model)) = (custom_prompt, model_id) {
                self.try_request_with_model(query, prompt, model).await
            } else {
                self.try_request(query).await
            };
            
            match result {
                Ok(result) => {
                    let filtered = if self.config.filter_thinking {
                        filter_thinking_content(&result)
                    } else {
                        result
                    };
                    return Ok(filtered);
                }
                Err(e) => {
                    if let AISearchError::Api { code, .. } = &e {
                        if retryable_codes.contains(code) && attempt < retry_count {
                            warn!("请求失败 (HTTP {}), 重试 {}/{}", code, attempt + 1, retry_count);
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                    
                    if attempt < retry_count {
                        warn!("请求失败, 重试 {}/{}", attempt + 1, retry_count);
                        sleep(Duration::from_secs(1)).await;
                        last_error = Some(e);
                        continue;
                    }
                    
                    return Err(e);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| AISearchError::Network {
            message: "未知错误".into(),
            suggestion: "请检查配置".into(),
        }))
    }
    
    /// 使用自定义系统提示词调用 API
    async fn call_api_with_custom_prompt(&self, query: &str, custom_prompt: &str) -> Result<String> {
        self.call_api_internal(query, Some(custom_prompt), Some(&self.config.search_model_id), self.config.analysis_retry_count).await
    }
    
    /// 使用指定模型和自定义系统提示词调用 API（用于查询分析）
    async fn call_api_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> Result<String> {
        self.call_api_internal(query, Some(custom_prompt), Some(model_id), self.config.analysis_retry_count).await
    }

    async fn call_api(&self, query: &str) -> Result<String> {
        self.call_api_internal(query, None, None, self.config.search_retry_count).await
    }
    

    async fn try_request_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> Result<String> {
        let endpoint = self.build_endpoint();
        let body = self.build_request_body_with_model(query, custom_prompt, model_id);
        
        let response = self.client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let status = response.status();
        
        if !status.is_success() {
            let status_code = status.as_u16();
            return Err(Self::handle_error_response(status_code, response).await);
        }
        
        if self.config.stream {
            self.handle_streaming_response(response).await
        } else {
            self.handle_json_response(response).await
        }
    }

    async fn try_request(&self, query: &str) -> Result<String> {
        let endpoint = self.build_endpoint();
        let body = self.build_request_body(query);
        
        let response = self.client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let status = response.status();
        
        if !status.is_success() {
            let status_code = status.as_u16();
            return Err(Self::handle_error_response(status_code, response).await);
        }
        
        if self.config.stream {
            self.handle_streaming_response(response).await
        } else {
            self.handle_json_response(response).await
        }
    }
    
    async fn handle_streaming_response(&self, response: reqwest::Response) -> Result<String> {
        const MAX_BUFFER_SIZE: usize = 10 * 1024 * 1024; // 10MB
        const MAX_CHUNKS_SIZE: usize = 10 * 1024 * 1024; // 10MB
        const MAX_LINE_BUFFER_SIZE: usize = 1024 * 1024; // 1MB - 防止单行过大
        
        let mut stream = response.bytes_stream();
        let mut chunks = Vec::new();
        let mut chunks_total_size = 0;
        let mut buffer = String::new();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let new_content = String::from_utf8_lossy(&chunk);
            
            // 在添加前检查大小,避免内存峰值
            if buffer.len() + new_content.len() > MAX_BUFFER_SIZE {
                return Err(AISearchError::Protocol(
                    format!("单次响应缓冲区过大，超过 {} MB 限制", MAX_BUFFER_SIZE / 1024 / 1024)
                ));
            }
            
            buffer.push_str(&new_content);
            
            // 处理完整的行
            while let Some(newline_pos) = buffer.find('\n') {
                let line = &buffer[..newline_pos];
                
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" {
                        buffer = buffer[newline_pos + 1..].to_string();
                        continue;
                    }
                    
                    if let Ok(parsed) = serde_json::from_str::<ChatResponse>(data) {
                        if let Some(choice) = parsed.choices.first() {
                            if let Some(delta) = &choice.delta {
                                if let Some(content) = &delta.content {
                                    // 检查累积内容大小
                                    chunks_total_size += content.len();
                                    if chunks_total_size > MAX_CHUNKS_SIZE {
                                        return Err(AISearchError::Protocol(
                                            format!("响应内容过大，超过 {} MB 限制", MAX_CHUNKS_SIZE / 1024 / 1024)
                                        ));
                                    }
                                    chunks.push(content.clone());
                                }
                            }
                        }
                    }
                }
                
                buffer = buffer[newline_pos + 1..].to_string();
            }
            
            // 防止 buffer 无限增长（没有换行符的情况）
            if buffer.len() > MAX_LINE_BUFFER_SIZE {
                tracing::warn!("行缓冲区过大（{}字节），可能缺少换行符，清空缓冲区", buffer.len());
                buffer.clear();
            }
        }
        
        Ok(chunks.join(""))
    }
    
    async fn handle_json_response(&self, response: reqwest::Response) -> Result<String> {
        let result: ChatResponse = response.json().await?;
        
        result.choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .ok_or_else(|| AISearchError::Protocol("响应格式错误".into()))
    }
    
    fn build_endpoint(&self) -> String {
        let mut url = self.config.api_url.clone();
        if !url.ends_with("/v1/chat/completions") {
            if url.ends_with('/') {
                url.push_str("v1/chat/completions");
            } else {
                url.push_str("/v1/chat/completions");
            }
        }
        url
    }
    

    fn build_request_body_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> ChatRequest {
        ChatRequest {
            model: model_id.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: custom_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: query.to_string(),
                },
            ],
            stream: self.config.stream,
        }
    }

    fn build_request_body(&self, query: &str) -> ChatRequest {
        let current_time = Local::now().format("%Y-%m-%d %H:%M:%S %A").to_string();
        let system_prompt = self.config.system_prompt.replace("{current_time}", &current_time);
        
        ChatRequest {
            model: self.config.search_model_id.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: query.to_string(),
                },
            ],
            stream: self.config.stream,
        }
    }
    
    /// 调用 AI 模型将查询拆分成多个子问题
    async fn split_query(&self, query: &str, count: u32) -> Result<Vec<String>> {
        let user_prompt = format!(
            r#"将查询拆分成 {} 个子问题，返回 JSON 数组。

查询: {}

只返回 JSON 数组，格式: ["子问题1", "子问题2", "子问题3"]"#,
            count, query
        );
        
        // 使用配置的拆分提示词
        let system_prompt = &self.config.split_prompt;
        
        // 使用分析模型（如果配置了）或默认模型
        let response = if let Some(analysis_model) = &self.config.analysis_model_id {
            tracing::debug!("使用分析模型拆分查询: {}", analysis_model);
            self.call_api_with_model(&user_prompt, system_prompt, analysis_model).await?
        } else {
            tracing::debug!("使用默认模型拆分查询: {}", self.config.search_model_id);
            self.call_api_with_custom_prompt(&user_prompt, system_prompt).await?
        };
        
        tracing::debug!("AI 返回的原始响应: {}", response);
        
        // 先尝试过滤 thinking 标签
        let filtered = filter_thinking_content(&response);
        
        // 如果过滤后为空，使用原始响应
        let content = if filtered.is_empty() {
            tracing::debug!("过滤后内容为空，使用原始响应");
            &response
        } else {
            &filtered
        };
        
        tracing::debug!("处理后的响应: {}", content);
        
        // 清理响应，移除可能的 markdown 代码块标记
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        
        tracing::debug!("清理后的响应: {}", cleaned);
        
        // 解析 JSON 数组
        let sub_queries: Vec<String> = serde_json::from_str(cleaned)
            .map_err(|e| {
                error!("JSON 解析失败，原始响应: {}", filtered);
                AISearchError::Protocol(format!("解析子查询失败: {}，响应内容: {}", e, cleaned))
            })?;
        
        if sub_queries.is_empty() {
            return Err(AISearchError::Protocol("未能拆分出任何子查询".into()));
        }
        
        if sub_queries.len() != count as usize {
            warn!("期望 {} 个子查询，实际得到 {}，继续执行", count, sub_queries.len());
        }
        
        // 为每个子查询添加 [SUB_QUERY] 前缀
        let prefixed_queries: Vec<String> = sub_queries
            .into_iter()
            .map(|q| format!("[SUB_QUERY] {}", q))
            .collect();
        
        Ok(prefixed_queries)
    }
}

fn filter_thinking_content(content: &str) -> String {
    let content = THINKING_PATTERN.replace_all(content, "");
    let content = WHITESPACE_PATTERN.replace_all(&content, "\n\n");
    content.trim().to_string()
}
