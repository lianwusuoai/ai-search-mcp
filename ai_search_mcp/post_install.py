#!/usr/bin/env python3
"""
AI Search MCP - 安装后自动部署钩子
在 pip install 或 pip install --upgrade 后自动检测并部署 Docker
"""
import sys
import os
from pathlib import Path
from typing import Optional
import io

# 修复 Windows 控制台编码问题
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

from .utils import run_command, check_docker, get_installed_version, get_docker_version


def find_project_root() -> Optional[Path]:
    """查找项目根目录（包含 docker-compose.yml）
    
    Returns:
        项目根目录路径，如果未找到则返回 None
    """
    # 1. 环境变量
    if "AI_SEARCH_MCP_ROOT" in os.environ:
        root = Path(os.environ["AI_SEARCH_MCP_ROOT"])
        if (root / "docker-compose.yml").exists():
            return root
    
    # 2. 当前目录
    current = Path.cwd()
    if (current / "docker-compose.yml").exists():
        return current
    
    # 3. 向上查找
    for _ in range(5):
        current = current.parent
        if (current / "docker-compose.yml").exists():
            return current
    
    # 4. 用户目录下的默认位置
    default_path = Path.home() / ".ai-search-mcp-project"
    if (default_path / "docker-compose.yml").exists():
        return default_path
    
    return None


def auto_deploy_silent() -> None:
    """静默自动部署（仅在需要时）"""
    # 检查 Docker
    if not check_docker():
        return  # Docker 不可用，静默跳过
    
    # 查找项目根目录
    project_root = find_project_root()
    if not project_root:
        return  # 未找到项目，静默跳过
    
    # 获取版本信息
    installed_version = get_installed_version()
    docker_version = get_docker_version()
    
    if not installed_version:
        return  # 无法获取版本，跳过
    
    # 判断是否需要部署
    need_deploy = False
    
    if docker_version is None:
        # Docker 容器未运行，首次部署
        need_deploy = True
    elif installed_version != docker_version:
        # 版本不一致，需要更新
        need_deploy = True
    
    # 执行部署
    if need_deploy:
        print(f"\n🔍 检测到 Docker 环境，自动部署 AI Search MCP (版本: {installed_version})...")
        
        # 切换到项目目录
        original_cwd = os.getcwd()
        os.chdir(project_root)
        
        try:
            # 停止旧容器
            run_command("docker-compose down", check=False, capture=False, timeout=60)
            
            # 构建并启动（使用国内镜像源，速度更快）
            print("🔨 构建并启动容器（已配置国内镜像源）...")
            success, _ = run_command(
                "docker-compose up -d --build", 
                check=False, capture=False, timeout=600
            )
            
            if success:
                print("✅ Docker 部署成功！")
                print("🌐 服务地址: http://localhost:11000")
                print("⚙️  配置界面: http://localhost:11000/config")
            else:
                print("⚠️  Docker 部署失败，请手动运行: ai-search-mcp-deploy")
        except Exception as e:
            print(f"⚠️  部署过程出错: {e}")
            print("💡 手动部署命令: ai-search-mcp-deploy")
        finally:
            # 恢复原始目录
            os.chdir(original_cwd)


def main() -> None:
    """主函数 - 安装后自动执行"""
    try:
        auto_deploy_silent()
    except Exception:
        # 静默失败，不影响安装
        pass


if __name__ == "__main__":
    main()
