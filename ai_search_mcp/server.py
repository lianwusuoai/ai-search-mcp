#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
AI 搜索 MCP 服务器 (通用版)
支持任何兼容 OpenAI API 格式的 AI 模型进行联网搜索
"""
import sys
import os
import json
import logging
from typing import Dict, Any, Optional

from .config import AIConfig, load_config
from .client import AIClient
from .exceptions import AISearchMCPError, ProtocolError

# 配置日志
logger = logging.getLogger(__name__)


def setup_logging(level: str = "INFO") -> None:
    """配置日志系统"""
    # 避免重复配置
    root_logger = logging.getLogger()
    if root_logger.handlers:
        return
    
    handler = logging.StreamHandler(sys.stderr)
    
    logging.basicConfig(
        level=getattr(logging, level.upper()),
        format='%(asctime)s [%(levelname)s] %(name)s: %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S',
        handlers=[handler]
    )
    # 设置第三方库日志级别
    logging.getLogger('urllib3').setLevel(logging.WARNING)
    logging.getLogger('requests').setLevel(logging.WARNING)

def handle_initialize(request: Dict[str, Any]) -> Dict[str, Any]:
    """
    处理 initialize 请求
    
    Args:
        request: MCP 初始化请求
        
    Returns:
        服务器能力和信息
    """
    return {
        'protocolVersion': '2024-11-05',
        'capabilities': {
            'tools': {}
        },
        'serverInfo': {
            'name': 'ai-search-mcp',
            'version': '1.0.0'
        }
    }


def handle_tools_list(request: Dict[str, Any], config: AIConfig) -> Dict[str, Any]:
    """
    处理 tools/list 请求
    
    Args:
        request: MCP 工具列表请求
        config: AI 配置对象
        
    Returns:
        可用工具列表
    """
    return {
        'tools': [{
            'name': 'search_with_ai',
            'description': f'使用 AI 模型 ({config.model_id}) 进行联网搜索,搜索效果优于普通搜索引擎。适用于需要最新信息、深度分析或复杂问题的场景。',
            'inputSchema': {
                'type': 'object',
                'properties': {
                    'query': {
                        'type': 'string',
                        'description': '搜索查询内容,可以是问题、关键词或需要查找的信息'
                    }
                },
                'required': ['query']
            }
        }]
    }


def handle_tools_call(request: Dict[str, Any], client: AIClient) -> Dict[str, Any]:
    """
    处理 tools/call 请求
    
    Args:
        request: MCP 工具调用请求
        client: AI 客户端实例
        
    Returns:
        工具执行结果
        
    Raises:
        ProtocolError: 未知工具名称
    """
    tool_name = request['params']['name']
    if tool_name == 'search_with_ai':
        query = request['params']['arguments']['query']
        logger.info(f"搜索查询: {query}")
        result = client.search(query)
        logger.info(f"搜索成功,返回 {len(result)} 字符")
        return {
            'content': [{
                'type': 'text',
                'text': result
            }]
        }
    else:
        raise ProtocolError(f"未知工具: {tool_name}")


def handle_request(request: Dict[str, Any], client: AIClient, config: AIConfig) -> Dict[str, Any]:
    """
    处理 MCP 请求
    
    Args:
        request: MCP 请求对象
        client: AI 客户端实例
        config: AI 配置对象
        
    Returns:
        请求处理结果
        
    Raises:
        ProtocolError: 未知请求方法
    """
    method = request.get('method')
    
    if method == 'initialize':
        return handle_initialize(request)
    elif method == 'tools/list':
        return handle_tools_list(request, config)
    elif method == 'tools/call':
        return handle_tools_call(request, client)
    else:
        raise ProtocolError(f"未知方法: {method}")

def send_response(response: Dict[str, Any]) -> None:
    """
    发送 MCP 响应到 stdout
    
    Args:
        response: MCP 响应对象
    """
    # 使用 ensure_ascii=True 将所有非 ASCII 字符转换为 \uXXXX 转义序列
    # 这样可以完全避开 Windows 编码问题
    output = json.dumps(response, ensure_ascii=True)
    
    # 使用官方 SDK 的方式：write + newline + flush
    sys.stdout.write(output)
    sys.stdout.write('\n')
    sys.stdout.flush()


def send_error(request_id: Optional[int], error: Exception) -> None:
    """发送错误响应
    
    Args:
        request_id: 请求 ID，如果无法获取则为 None
        error: 异常对象
    """
    error_response = {
        'jsonrpc': '2.0',
        'id': request_id,
        'error': {
            'code': -32603,
            'message': str(error)
        }
    }
    send_response(error_response)


def run_server(config: AIConfig) -> None:
    """
    运行 MCP 服务器主循环
    
    Args:
        config: AI 配置对象
    """
    logger.info(f"启动 AI Search MCP Server v1.0.0")
    logger.info(f"API URL: {config.api_url}")
    logger.info(f"模型: {config.model_id}")
    logger.info(f"流式响应: {config.stream}")
    logger.info(f"超时时间: {config.timeout}秒")
    logger.info(f"过滤思考内容: {config.filter_thinking}")
    
    # 使用上下文管理器自动管理资源
    with AIClient(config) as client:
        for line in sys.stdin:
            request_id = None
            try:
                request = json.loads(line.strip())
                request_id = request.get('id')
                logger.info(f"收到请求: {request.get('method')}")
                
                result = handle_request(request, client, config)
                
                response = {
                    'jsonrpc': '2.0',
                    'id': request_id,
                    'result': result
                }
                
                send_response(response)
                
            except AISearchMCPError as e:
                logger.error(f"错误: {e.message}")
                if e.suggestion:
                    logger.error(f"建议: {e.suggestion}")
                send_error(request_id, e)
            except Exception as e:
                logger.error(f"未预期的错误: {str(e)}")
                send_error(request_id, e)



if __name__ == '__main__':
    from .cli import main
    main()
