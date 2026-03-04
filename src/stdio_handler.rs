use crate::client::AIClient;
use crate::config::AIConfig;
use crate::error::{AISearchError, Result};
use crate::error_codes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize)]
struct ServerInfo {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: Capabilities,
    #[serde(rename = "serverInfo")]
    server_info: ServerInfoDetails,
}

#[derive(Debug, Serialize)]
struct Capabilities {
    tools: Value,
}

#[derive(Debug, Serialize)]
struct ServerInfoDetails {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct ToolsList {
    tools: Vec<Tool>,
}

#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

#[derive(Debug, Serialize)]
struct ToolResult {
    content: Vec<Content>,
}

#[derive(Debug, Serialize)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

pub struct MCPServer {
    config: AIConfig,
    client: AIClient,
}

impl MCPServer {
    pub fn new(config: AIConfig) -> Result<Self> {
        let client = AIClient::new(config.clone())?;
        Ok(Self { config, client })
    }
    
    pub async fn run(&self) -> Result<()> {
        tracing::info!("启动 AI Search MCP Server v{}", env!("CARGO_PKG_VERSION"));
        tracing::info!("API URL: {}", self.config.api_url);
        tracing::info!("模型: {}", self.config.search_model_id);
        tracing::info!("流式响应: {}", self.config.stream);
        tracing::info!("超时时间: {}秒", self.config.timeout);
        tracing::info!("过滤思考内容: {}", self.config.filter_thinking);
        tracing::info!("日志级别: {}", self.config.log_level);
        
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();
        
        while let Some(line) = reader.next_line().await? {
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    tracing::error!("JSON 解析错误: {}", e);
                    continue;
                }
            };
            
            tracing::debug!("收到请求: {}", request.method);
            
            let response = match self.handle_request(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::error!("错误: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: error_codes::INTERNAL_ERROR,
                            message: e.to_string(),
                        }),
                    }
                }
            };
            
            let output = serde_json::to_string(&response)?;
            stdout.write_all(output.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
        
        Ok(())
    }
    
    async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params).await?,
            _ => return Err(AISearchError::Protocol(format!("未知方法: {}", request.method))),
        };
        
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        })
    }
    
    fn handle_initialize(&self) -> Value {
        serde_json::to_value(ServerInfo {
            protocol_version: "2024-11-05".to_string(),
            capabilities: Capabilities {
                tools: serde_json::json!({}),
            },
            server_info: ServerInfoDetails {
                name: "ai-search-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }).unwrap()
    }
    
    fn handle_tools_list(&self) -> Value {
        let description = if self.config.max_query_plan == 1 {
            format!("使用 AI 模型 ({}) 进行网络搜索。直接搜索用户查询，返回详细的搜索结果。", self.config.search_model_id)
        } else {
            format!(
                "使用 AI 模型 ({}) 进行网络搜索。\n\n多维度搜索模式：服务端自动将查询拆分成 {} 个子问题并并发执行，直接返回所有子查询结果（不整合）。单次调用即可完成多维度搜索。",
                self.config.search_model_id,
                self.config.max_query_plan
            )
        };
        
        serde_json::to_value(ToolsList {
            tools: vec![Tool {
                name: "ai_search".to_string(),
                description,
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "搜索查询内容"
                        }
                    },
                    "required": ["query"]
                }),
            }],
        }).unwrap()
    }
    
    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value> {
        let params = params.ok_or_else(|| AISearchError::Protocol("缺少参数".into()))?;
        
        let tool_name = params["name"]
            .as_str()
            .ok_or_else(|| AISearchError::Protocol("缺少工具名称".into()))?;
        
        if tool_name != "ai_search" {
            return Err(AISearchError::Protocol(format!("未知工具: {}", tool_name)));
        }
        
        let query = params["arguments"]["query"]
            .as_str()
            .ok_or_else(|| AISearchError::Protocol("缺少查询参数".into()))?;
        
        tracing::info!("搜索查询: {}", query);
        let result = self.client.search(query).await?;
        tracing::info!("搜索成功,返回 {} 字符", result.len());
        
        Ok(serde_json::to_value(ToolResult {
            content: vec![Content {
                content_type: "text".to_string(),
                text: result,
            }],
        }).unwrap())
    }
}
