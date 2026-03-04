"""AI Search MCP Server - Python Wrapper for Rust Binary"""

import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import NoReturn

from .__about__ import __version__


def get_binary_name() -> str:
    """获取当前平台的二进制文件名"""
    system = platform.system().lower()
    
    if system == "windows":
        return "ai-search-mcp.exe"
    elif system == "linux":
        return "ai-search-mcp-linux"
    elif system == "darwin":
        return "ai-search-mcp-macos"
    else:
        raise RuntimeError(f"不支持的操作系统: {system}")


def get_binary_path() -> Path:
    """获取二进制文件的完整路径"""
    binary_name = get_binary_name()
    binary_path = Path(__file__).parent / "bin" / binary_name
    
    if not binary_path.exists():
        raise FileNotFoundError(
            f"未找到二进制文件: {binary_path}\n"
            f"请确保已正确安装 ai-search-mcp 包"
        )
    
    # 确保二进制文件有执行权限（Unix 系统）
    if platform.system() != "Windows":
        os.chmod(binary_path, 0o755)
    
    return binary_path


def check_and_deploy_docker() -> None:
    """检查并自动部署 Docker（仅首次或版本变化时）"""
    try:
        # 检查是否已经部署过当前版本
        marker_file = Path.home() / ".ai-search-mcp" / ".deployed_version"
        
        # 读取已部署的版本
        deployed_version = None
        if marker_file.exists():
            try:
                deployed_version = marker_file.read_text().strip()
            except (IOError, OSError) as e:
                # 文件读取失败，记录但继续
                pass
        
        # 如果版本一致，跳过检测
        if deployed_version == __version__:
            return
        
        # 导入部署模块（延迟导入，避免影响启动速度）
        from .post_install import auto_deploy_silent
        
        # 执行自动部署
        auto_deploy_silent()
        
        # 标记已部署的版本
        try:
            marker_file.parent.mkdir(parents=True, exist_ok=True)
            marker_file.write_text(__version__)
        except (IOError, OSError, PermissionError) as e:
            # 标记文件写入失败，记录但不影响主程序
            pass
            
    except ImportError as e:
        # 导入失败，可能是依赖问题
        pass
    except Exception as e:
        # 其他异常，静默失败
        pass


def main() -> NoReturn:
    """主入口函数，调用 Rust 二进制文件
    
    默认行为:
    - 无参数: 启动 stdio 模式 (用于 MCP 客户端集成)
    - --http: 启动 HTTP 模式 (用于 REST API)
    - --http --port <PORT>: 自定义端口的 HTTP 模式
    """
    try:
        # 首次运行或版本更新时，自动检测并部署 Docker
        # 这个检查非常快（只读取一个文件），不会影响启动速度
        check_and_deploy_docker()
        
        binary_path = get_binary_path()
        
        # 获取命令行参数
        args = sys.argv[1:]
        
        # 默认使用 stdio 模式 (MCP 标准)
        # 用户可以通过参数选择其他模式：
        # - ai-search-mcp (stdio 模式，用于 Kiro/Claude Desktop)
        # - ai-search-mcp --http (HTTP 模式)
        # - ai-search-mcp --http --port 8080 (自定义端口)
        
        # 直接执行二进制文件，传递所有参数
        result = subprocess.run(
            [str(binary_path)] + args,
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
        )
        
        sys.exit(result.returncode)
        
    except FileNotFoundError as e:
        print(f"错误: {e}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"执行失败: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
