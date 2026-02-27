# AI Search MCP Server

[![PyPI version](https://badge.fury.io/py/ai-search-mcp.svg)](https://badge.fury.io/py/ai-search-mcp)
[![Python versions](https://img.shields.io/pypi/pyversions/ai-search-mcp.svg)](https://pypi.org/project/ai-search-mcp/)
[![License](https://img.shields.io/pypi/l/ai-search-mcp.svg)](https://github.com/lianwusuoai/ai-search-mcp/blob/main/LICENSE)

通用 AI 搜索 MCP 服务器，支持任何兼容 OpenAI API 格式的 AI 模型进行联网搜索。

## 特性

- 支持任何 OpenAI API 兼容的模型
- 支持流式和非流式响应
- 自动过滤 AI 思考内容
- 完全可配置
- Windows 平台完美支持中文显示

## 安装

```bash
# 使用 uvx（推荐）
uvx ai-search-mcp

# 或使用 pip
pip install ai-search-mcp
```

## 更新

```bash
# 使用 pip 更新
pip install --upgrade ai-search-mcp

# 使用 uvx 会自动使用最新版本
uvx ai-search-mcp
```

## 卸载

```bash
# 使用 pip 卸载
pip uninstall ai-search-mcp

# uvx 不需要卸载，每次运行都是独立环境
```

## 快速开始

### Kiro IDE

编辑 `.kiro/settings/mcp.json`:

```json
{
  "mcpServers": {
    "ai-search": {
      "command": "uvx",
      "args": ["ai-search-mcp"],
      "env": {
        "AI_API_URL": "http://localhost:10000",
        "AI_API_KEY": "your-api-key",
        "AI_MODEL_ID": "Grok"
      }
    }
  }
}
```

### Claude Desktop

编辑 `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ai-search": {
      "command": "uvx",
      "args": ["ai-search-mcp"],
      "env": {
        "AI_API_URL": "http://localhost:10000",
        "AI_API_KEY": "your-api-key",
        "AI_MODEL_ID": "Grok"
      }
    }
  }
}
```

## 配置

### 必需环境变量

| 变量 | 说明 |
|------|------|
| `AI_API_URL` | AI API 地址 |
| `AI_API_KEY` | API 密钥 |
| `AI_MODEL_ID` | 模型 ID |

### 可选环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `AI_TIMEOUT` | `60` | 超时时间（秒），复杂查询建议设为 120 |
| `AI_STREAM` | `true` | 流式响应 |
| `AI_FILTER_THINKING` | `true` | 过滤思考内容 |

### 超时配置建议

如果遇到复杂查询超时，可以增加超时时间：

```json
{
  "mcpServers": {
    "ai-search": {
      "command": "uvx",
      "args": ["ai-search-mcp"],
      "env": {
        "AI_API_URL": "http://localhost:10000",
        "AI_API_KEY": "your-api-key",
        "AI_MODEL_ID": "Grok",
        "AI_TIMEOUT": "120"
      }
    }
  }
}
```

## 支持的服务

任何兼容 OpenAI API 格式的服务都可以使用，例如：

- Grok（本地部署）
- OpenAI（GPT-4、GPT-3.5）
- 本地模型（Ollama、LM Studio）
- 其他兼容服务

## 命令行工具

```bash
# 查看版本
ai-search-mcp --version

# 验证配置
ai-search-mcp --validate-config
```

## 开发

```bash
git clone https://github.com/lianwusuoai/ai-search-mcp.git
cd ai-search-mcp
pip install -e .
```

## 许可证

MIT License

## 链接

- [GitHub](https://github.com/lianwusuoai/ai-search-mcp)
- [PyPI](https://pypi.org/project/ai-search-mcp/)
- [问题反馈](https://github.com/lianwusuoai/ai-search-mcp/issues)
