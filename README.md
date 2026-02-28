# AI Search MCP Server

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/lianwusuoai/ai-search-mcp/blob/main/LICENSE)

通用 AI 搜索 MCP 服务器，支持任何兼容 OpenAI API 格式的 AI 模型进行联网搜索。

**高性能 Rust 实现，支持真正的并发搜索。**

## 特性

- ✅ 支持任何 OpenAI API 兼容的模型
- ✅ 支持流式和非流式响应
- ✅ 自动过滤 AI 思考内容
- ✅ **自动时间注入**：每次搜索自动注入当前时间，提升时间相关查询准确性
- ✅ **增强系统提示词**：内置优化的搜索策略和输出要求
- ✅ **多维度搜索**：自动拆分复杂查询为多个子问题并行搜索，结果更全面
- ✅ **智能重试机制**：自动重试失败的请求，提升成功率
- ✅ **高性能并发**：基于 Tokio 异步运行时，真正的并发执行
- ✅ **零依赖部署**：单一二进制文件，无需 Python 环境
- ✅ 完全可配置（支持自定义系统提示词）

## 性能对比

| 特性 | Python 版本 | Rust 版本 |
|------|------------|-----------|
| 多维度搜索 | 顺序执行 (~60s每次) | 并发执行 (一共60s) |
| 启动时间 | ~200ms | ~10ms |
| 内存占用 | 中等 | 低 (零拷贝) |
| 流式响应 | iter_lines | Stream trait |
| 依赖 | Python + requests | 单一二进制文件 |


## 安装

### 方式一：使用 Python 包管理器（推荐）

```bash
# 使用 uvx（推荐）
uvx ai-search-mcp

# 或使用 pip
pip install ai-search-mcp
```

### 方式二：从源码编译

```bash
# 克隆仓库
git clone https://github.com/lianwusuoai/ai-search-mcp.git
cd ai-search-mcp

# 编译发布版本
cargo build --release

# 二进制文件位于
./target/release/ai-search-mcp
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


#### 跨平台编译

```bash
# Windows
cargo build --release --target x86_64-pc-windows-gnu

# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# macOS
cargo build --release --target x86_64-apple-darwin
```

## 快速开始

编辑配置文件（Kiro IDE: `.kiro/settings/mcp.json` | Claude Desktop: `claude_desktop_config.json`）:

```json
{
  "mcpServers": {
    "ai-search": {
      "command": "uvx",  // 使用 uvx（推荐）
      "args": ["ai-search-mcp"],  // 或使用绝对路径: "/path/to/ai-search-mcp"
      "env": {
        "AI_API_URL": "http://localhost:10000",
        "AI_API_KEY": "your-api-key",
        "AI_MODEL_ID": "搜索模型ID",
        "AI_ANALYSIS_MODEL_ID": "分析模型ID",
        "AI_TIMEOUT": "60",
        "AI_STREAM": "true",
        "AI_FILTER_THINKING": "true",
        "AI_RETRY_COUNT": "1",
        "AI_LOG_LEVEL": "INFO",
        "AI_MAX_QUERY_PLAN": "3"
      }
    }
  }
}
```

## 工具说明

### `web_search` - 网络搜索

**输入**：`{"query": "搜索内容"}`

**多维度搜索**（由 `AI_MAX_QUERY_PLAN` 控制）：
- `= 1`：直接返回搜索结果
- `> 1`：首次调用返回拆分要求，AI 需拆分成 N 个子问题并行搜索（子问题加 `[SUB_QUERY]` 前缀防止套娃），然后整合结果

---

## 配置说明

### 环境变量

| 变量 | 必需 | 默认值 | 说明 |
|------|------|--------|------|
| `AI_API_URL` | ✅ | - | AI API 地址 |
| `AI_API_KEY` | ✅ | - | API 密钥 |
| `AI_MODEL_ID` | ✅ | - | 搜索查询生成模型 ID |
| `AI_ANALYSIS_MODEL_ID` | ❌ | 同 `AI_MODEL_ID` | 搜索结果分析模型 ID（可与搜索模型不同） |
| `AI_TIMEOUT` | ❌ | `60` | 超时时间（秒），复杂查询建议 120 |
| `AI_STREAM` | ❌ | `true` | 是否启用流式响应 |
| `AI_FILTER_THINKING` | ❌ | `true` | 是否过滤思考内容 |
| `AI_RETRY_COUNT` | ❌ | `1` | 重试次数（0 = 不重试） |
| `AI_LOG_LEVEL` | ❌ | `INFO` | 日志级别（DEBUG/INFO/WARNING/ERROR） |
| `AI_MAX_QUERY_PLAN` | ❌ | `1` | 复杂查询拆分维度数（建议 3-7） |
| `AI_SYSTEM_PROMPT` | ❌ | 见下方 | 自定义系统提示词 |


### 搜索AI自定义提示词示例

**重要**：必须保留 `{current_time}` 占位符

```bash
# 简化版
export AI_SYSTEM_PROMPT="你是搜索助手。当前时间: {current_time}。请提供准确答案并标注来源。"

# 技术文档专用
export AI_SYSTEM_PROMPT="你是技术文档搜索专家。当前时间: {current_time}。专注于官方文档、GitHub 仓库和技术博客，提供代码示例并标注版本信息。"
```

---

## 多维度搜索示例

### 简单查询（AI_MAX_QUERY_PLAN = 1）
用户：Python 是什么  
→ AI 调用：`web_search("Python 是什么")`  
→ MCP 返回：直接返回搜索结果

### 复杂查询（AI_MAX_QUERY_PLAN = 3）
用户：春节北京到上海高铁票价  
→ AI 首次调用：`web_search("春节北京到上海高铁票价")`  
→ MCP 返回：拆分要求（提示拆成 3 个子问题）  
→ AI 并行调用：
```
web_search("[SUB_QUERY] 春节北京到上海直达高铁票价")
web_search("[SUB_QUERY] 北京到上海中转方案票价对比")
web_search("[SUB_QUERY] 北京周边站点到上海买长乘短策略")
```
→ MCP 返回：每个子查询的搜索结果（`[SUB_QUERY]` 前缀防止再次拆分）  
→ AI 整合：自动整合 3 个结果，返回完整答案

**性能提升**：Rust 版本并发执行 3 个子查询，总耗时约 1 秒（Python 顺序执行约 3 秒）

---
## 命令行工具

```bash
# 查看版本
ai-search-mcp --version

# 验证配置
ai-search-mcp --validate-config
```

## 支持的服务

任何兼容 OpenAI API 格式的服务都可以使用，以及本地部署的AI模型

## 开发

```bash
# 克隆仓库
git clone https://github.com/lianwusuoai/ai-search-mcp.git
cd ai-search-mcp

# 开发构建
cargo build

# 运行测试
cargo test

# 发布构建
cargo build --release
```

## 许可证

MIT License

## 链接

- [GitHub](https://github.com/lianwusuoai/ai-search-mcp)
- [问题反馈](https://github.com/lianwusuoai/ai-search-mcp/issues)
