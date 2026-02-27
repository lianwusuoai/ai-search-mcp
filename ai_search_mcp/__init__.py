"""
AI Search MCP Server

通用 AI 搜索 MCP 服务器，支持任何兼容 OpenAI API 格式的 AI 模型。
"""

from .cli import main
from .config import AIConfig, load_config
from .client import AIClient
from .exceptions import (
    AISearchMCPError,
    ConfigError,
    MissingConfigError,
    InvalidConfigError,
    APIError,
    NetworkError,
    TimeoutError,
    ProtocolError,
)

__version__ = "1.0.0"
__all__ = [
    "main",
    "AIConfig",
    "load_config",
    "AIClient",
    "AISearchMCPError",
    "ConfigError",
    "MissingConfigError",
    "InvalidConfigError",
    "APIError",
    "NetworkError",
    "TimeoutError",
    "ProtocolError",
]
