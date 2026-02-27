"""AI 客户端模块"""
import re
import json
from typing import Optional
import requests

from .config import AIConfig
from .exceptions import APIError, NetworkError, TimeoutError

# 常量定义
SSE_DATA_PREFIX = 'data: '
SSE_DATA_PREFIX_LEN = len(SSE_DATA_PREFIX)
SSE_DONE_MESSAGE = '[DONE]'

# 预编译正则表达式
THINKING_PATTERN = re.compile(
    r'<think(?:ing)?>.*?</think(?:ing)?>',
    re.DOTALL | re.IGNORECASE
)
WHITESPACE_PATTERN = re.compile(r'\n\s*\n')


class AIClient:
    """
    AI API 客户端
    
    支持上下文管理器协议，自动管理资源。
    
    Example:
        with AIClient(config) as client:
            result = client.search("query")
    """
    
    def __init__(self, config: AIConfig):
        """
        初始化客户端
        
        Args:
            config: AI 配置对象
        """
        self.config = config
        self.session = requests.Session()
        self.session.headers.update({
            'Authorization': f'Bearer {config.api_key}',
            'Content-Type': 'application/json'
        })
    
    def __enter__(self) -> 'AIClient':
        """进入上下文管理器"""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """退出上下文管理器，关闭 session"""
        self.close()
    
    def close(self) -> None:
        """关闭 session 释放资源"""
        if self.session:
            self.session.close()
    
    def search(self, query: str) -> str:
        """
        执行搜索查询
        
        Args:
            query: 搜索查询内容
            
        Returns:
            搜索结果文本
            
        Raises:
            APIError: API 调用失败
            NetworkError: 网络连接失败
            TimeoutError: 请求超时
        """
        try:
            endpoint = self._build_endpoint()
            body = self._build_request_body(query)
            
            response = self.session.post(
                endpoint,
                json=body,
                stream=self.config.stream,
                timeout=self.config.timeout
            )
            
            if response.status_code == 200:
                if self.config.stream:
                    result = self._handle_streaming_response(response)
                else:
                    result = self._handle_json_response(response)
                
                # 根据配置决定是否过滤思考内容
                if self.config.filter_thinking:
                    result = self._filter_thinking_content(result)
                
                return result
            else:
                # 处理 API 错误
                detail = response.text
                if response.status_code == 401:
                    detail = "认证失败,请检查 API_KEY 是否正确"
                elif response.status_code == 429:
                    detail = "请求过于频繁,请稍后重试"
                elif response.status_code >= 500:
                    detail = "服务器错误,请稍后重试"
                
                raise APIError(response.status_code, detail)
                
        except requests.exceptions.ConnectionError as e:
            raise NetworkError(
                f"无法连接到 API 服务器: {self.config.api_url}",
                "请检查: 1) API 地址是否正确 2) 网络连接是否正常 3) 服务器是否运行"
            )
        except requests.exceptions.Timeout:
            raise TimeoutError(self.config.timeout)
        except (APIError, NetworkError, TimeoutError):
            # 重新抛出项目异常
            raise
        except Exception as e:
            # 捕获其他异常并转换
            raise NetworkError(
                f"请求失败: {str(e)}",
                "请检查网络连接和配置"
            )
    
    def _build_endpoint(self) -> str:
        """构建 API 端点 URL"""
        api_url = self.config.api_url
        if not api_url.endswith('/v1/chat/completions'):
            if api_url.endswith('/'):
                api_url += 'v1/chat/completions'
            else:
                api_url += '/v1/chat/completions'
        return api_url
    
    def _build_request_body(self, query: str) -> dict:
        """构建请求体"""
        return {
            'model': self.config.model_id,
            'messages': [
                {
                    'role': 'system',
                    'content': self.config.system_prompt
                },
                {
                    'role': 'user',
                    'content': query
                }
            ],
            'stream': self.config.stream
        }
    
    def _handle_streaming_response(self, response: requests.Response) -> str:
        """处理流式响应"""
        chunks = []
        response.encoding = 'utf-8'
        for line in response.iter_lines(decode_unicode=True):
            if line and line.startswith('data: '):
                content = self._parse_sse_line(line)
                if content:
                    chunks.append(content)
        return ''.join(chunks)
    
    def _handle_json_response(self, response: requests.Response) -> str:
        """处理 JSON 响应"""
        try:
            result = response.json()
            return result['choices'][0]['message']['content']
        except (json.JSONDecodeError, KeyError) as e:
            raise APIError(
                response.status_code,
                f"响应格式错误: {str(e)}"
            )
    
    def _parse_sse_line(self, line: str) -> Optional[str]:
        """
        解析 SSE 数据行
        
        Args:
            line: SSE 数据行
            
        Returns:
            提取的内容，如果无内容则返回 None
        """
        data_str = line[SSE_DATA_PREFIX_LEN:]  # 移除 'data: ' 前缀
        if data_str.strip() == SSE_DONE_MESSAGE:
            return None
        
        try:
            data = json.loads(data_str)
            if 'choices' in data and len(data['choices']) > 0:
                delta = data['choices'][0].get('delta', {})
                return delta.get('content', '')
        except json.JSONDecodeError:
            pass
        
        return None
    
    def _filter_thinking_content(self, content: str) -> str:
        """
        过滤思考内容
        
        移除 <think>...</think> 和 <thinking>...</thinking> 标签及其内容
        
        Args:
            content: 原始内容
            
        Returns:
            过滤后的内容
        """
        # 使用预编译的正则表达式
        content = THINKING_PATTERN.sub('', content)
        # 清理多余的空白
        content = WHITESPACE_PATTERN.sub('\n\n', content)
        return content.strip()
