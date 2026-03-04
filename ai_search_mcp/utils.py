"""AI Search MCP - 公共工具函数"""
import subprocess
from typing import Tuple, Optional


def run_command(
    cmd: str, 
    check: bool = True, 
    capture: bool = True, 
    cwd: Optional[str] = None,
    timeout: int = 120
) -> Tuple[bool, str]:
    """执行命令
    
    Args:
        cmd: 要执行的命令
        check: 是否检查返回码
        capture: 是否捕获输出
        cwd: 工作目录
        timeout: 超时时间（秒）
        
    Returns:
        (成功标志, 输出内容)
    """
    try:
        if capture:
            result = subprocess.run(
                cmd, shell=True, check=check, 
                capture_output=True, text=True, timeout=timeout, cwd=cwd
            )
            return result.returncode == 0, result.stdout.strip()
        else:
            result_bytes = subprocess.run(
                cmd, shell=True, check=check, timeout=timeout, cwd=cwd
            )
            return result_bytes.returncode == 0, ""
    except subprocess.TimeoutExpired:
        return False, "命令超时"
    except subprocess.CalledProcessError as e:
        return False, str(e)
    except Exception as e:
        return False, str(e)


def check_docker() -> bool:
    """检查 Docker 是否运行
    
    Returns:
        Docker 是否可用
    """
    success, _ = run_command("docker info", check=False)
    return success


def get_installed_version() -> str:
    """获取当前安装的版本
    
    Returns:
        版本号
    """
    from . import __about__
    return __about__.__version__


def get_docker_version() -> Optional[str]:
    """获取 Docker 容器版本
    
    Returns:
        容器版本号，如果容器未运行则返回 None
    """
    try:
        import urllib.request
        import json
        from typing import Any
        with urllib.request.urlopen("http://localhost:11000/health", timeout=2) as response:
            data: Any = json.loads(response.read().decode())
            version: Optional[str] = data.get("version")
            return version
    except Exception:
        return None
