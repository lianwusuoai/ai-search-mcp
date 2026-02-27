"""命令行接口模块"""
import sys
import argparse
import logging
from typing import Optional

from .config import AIConfig, load_config
from .server import run_server, setup_logging
from .exceptions import AISearchMCPError


def parse_args() -> argparse.Namespace:
    """解析命令行参数"""
    parser = argparse.ArgumentParser(
        prog='ai-search-mcp',
        description='通用 AI 搜索 MCP 服务器'
    )
    parser.add_argument(
        '--version',
        action='version',
        version='%(prog)s 1.0.0'
    )
    parser.add_argument(
        '--validate-config',
        action='store_true',
        help='验证配置并显示摘要'
    )
    parser.add_argument(
        '--config',
        type=str,
        help='配置文件路径'
    )
    parser.add_argument(
        '--log-level',
        type=str,
        choices=['DEBUG', 'INFO', 'WARNING', 'ERROR'],
        default='INFO',
        help='日志级别'
    )
    return parser.parse_args()


def mask_sensitive(value: str, show_chars: int = 4) -> str:
    """
    遮蔽敏感信息
    
    Args:
        value: 需要遮蔽的字符串
        show_chars: 显示的字符数（前后各显示这么多）
        
    Returns:
        遮蔽后的字符串
    """
    # 短于 12 字符的完全遮蔽
    if len(value) < 12:
        return '***'
    # 长度在 12-16 之间，只显示前后各 2 个字符
    elif len(value) <= 16:
        return f"{value[:2]}...{value[-2:]}"
    # 长度超过 16，显示前后各 4 个字符
    else:
        return f"{value[:show_chars]}...{value[-show_chars:]}"


def show_config_summary(config: AIConfig) -> None:
    """显示配置摘要(隐藏敏感信息)"""
    logger = logging.getLogger(__name__)
    logger.info("配置摘要:")
    logger.info(f"  API URL: {config.api_url}")
    logger.info(f"  API Key: {mask_sensitive(config.api_key)}")
    logger.info(f"  模型: {config.model_id}")
    logger.info(f"  超时: {config.timeout}秒")
    logger.info(f"  流式响应: {config.stream}")
    logger.info(f"  过滤思考内容: {config.filter_thinking}")


def validate_config_command(config_file: Optional[str]) -> None:
    """验证配置命令"""
    try:
        config = load_config(config_file)
        show_config_summary(config)
        print("\n✓ 配置验证成功")
        sys.exit(0)
    except AISearchMCPError as e:
        logger = logging.getLogger(__name__)
        logger.error(f"配置错误: {e.message}")
        if e.suggestion:
            logger.error(f"建议: {e.suggestion}")
        sys.exit(1)


def main() -> None:
    """CLI 入口点"""
    args = parse_args()
    
    # 配置日志
    setup_logging(args.log_level)
    
    # 处理验证配置命令
    if args.validate_config:
        validate_config_command(args.config)
        return
    
    # 正常启动服务器
    try:
        config = load_config(args.config)
        run_server(config)
    except AISearchMCPError as e:
        logger = logging.getLogger(__name__)
        logger.error(f"启动失败: {e.message}")
        if e.suggestion:
            logger.error(f"建议: {e.suggestion}")
        sys.exit(1)
    except Exception as e:
        logger = logging.getLogger(__name__)
        logger.error(f"未预期的错误: {str(e)}")
        sys.exit(1)


if __name__ == '__main__':
    main()
