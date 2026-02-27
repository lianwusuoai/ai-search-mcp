"""自定义异常类"""
from typing import Optional


class AISearchMCPError(Exception):
    """基础异常类"""
    
    def __init__(self, message: str, suggestion: Optional[str] = None):
        self.message = message
        self.suggestion = suggestion
        super().__init__(self.format_message())
    
    def format_message(self) -> str:
        """格式化错误消息"""
        if self.suggestion:
            return f"{self.message}\n建议: {self.suggestion}"
        return self.message


class ConfigError(AISearchMCPError):
    """配置错误"""
    pass


class MissingConfigError(ConfigError):
    """缺失必需配置"""
    
    def __init__(self, key: str, example: str):
        message = f"缺少必需的配置项: {key}"
        suggestion = f"请设置环境变量或在配置文件中添加: {example}"
        super().__init__(message, suggestion)


class InvalidConfigError(ConfigError):
    """无效配置"""
    pass


class APIError(AISearchMCPError):
    """API 调用错误"""
    
    def __init__(self, status_code: int, detail: str):
        message = f"API 返回错误: HTTP {status_code}"
        suggestion = f"错误详情: {detail}"
        super().__init__(message, suggestion)


class NetworkError(AISearchMCPError):
    """网络连接错误"""
    pass


class TimeoutError(AISearchMCPError):
    """请求超时"""
    
    def __init__(self, timeout: int):
        message = f"请求超时 ({timeout}秒)"
        suggestion = "请检查网络连接和 API 地址是否正确"
        super().__init__(message, suggestion)


class ProtocolError(AISearchMCPError):
    """MCP 协议错误"""
    pass
