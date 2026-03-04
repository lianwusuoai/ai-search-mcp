#!/usr/bin/env python3
"""
AI Search MCP - Docker 部署管理工具
"""
import sys
import argparse
from pathlib import Path
from typing import Optional
import io

# 修复 Windows 控制台编码问题
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

from .utils import run_command, check_docker, get_installed_version, get_docker_version
def find_project_root() -> Optional[Path]:
    """查找项目根目录
    
    Returns:
        项目根目录路径，如果未找到则返回 None
    """
    import os
    
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
    
    return None


def deploy(force: bool = False, stop: bool = False) -> int:
    """部署 Docker 容器
    
    Args:
        force: 是否强制重新部署
        stop: 是否停止服务
        
    Returns:
        退出码（0表示成功）
    """
    print("🔍 AI Search MCP - Docker 部署管理")
    print()
    
    # 检查 Docker
    if not check_docker():
        print("❌ Docker 未运行，请先启动 Docker Desktop")
        return 1
    
    # 停止服务
    if stop:
        project_root = find_project_root()
        if not project_root:
            print("❌ 未找到 docker-compose.yml")
            return 1
        
        print("⏹️  停止 Docker 服务...")
        run_command("docker-compose down", check=False, capture=False, cwd=str(project_root))
        print("✅ 服务已停止")
        return 0
    
    # 查找项目根目录
    project_root = find_project_root()
    if not project_root:
        print("❌ 未找到 docker-compose.yml")
        print()
        print("💡 解决方案：")
        print("   1. 在项目目录运行此命令")
        print("   2. 设置环境变量: export AI_SEARCH_MCP_ROOT=/path/to/project")
        print("   3. 克隆项目: git clone https://github.com/lianwusuoai/ai-search-mcp.git")
        return 1
    
    print(f"📁 项目目录: {project_root}")
    
    # 获取版本信息
    installed_version = get_installed_version()
    docker_version = get_docker_version()
    
    print(f"📦 已安装版本: {installed_version}")
    print(f"🐳 Docker 版本: {docker_version or '未运行'}")
    print()
    
    # 判断是否需要部署
    need_deploy = False
    
    if force:
        print("⚠️  强制重新部署模式")
        need_deploy = True
    elif docker_version is None:
        print("ℹ️  Docker 容器未运行，准备首次部署")
        need_deploy = True
    elif installed_version != docker_version:
        print("⚠️  版本不一致，准备更新 Docker 容器")
        need_deploy = True
    else:
        print("✅ 版本一致，无需更新")
        print(f"🌐 服务地址: http://localhost:11000")
        return 0
    
    # 执行部署
    if need_deploy:
        print()
        print("🚀 开始部署...")
        
        # 停止旧容器
        print("⏹️  停止旧容器...")
        run_command("docker-compose down", check=False, capture=False, cwd=str(project_root))
        
        # 构建并启动
        print(f"🔨 构建并启动新容器（版本: {installed_version}）...")
        success, output = run_command(
            "docker-compose up -d --build", 
            check=False, capture=False, cwd=str(project_root)
        )
        
        if success:
            print()
            print("✅ 部署成功！")
            print("🌐 服务地址: http://localhost:11000")
            print("⚙️  配置界面: http://localhost:11000/config")
            print("💚 健康检查: http://localhost:11000/health")
            return 0
        else:
            print()
            print("❌ 部署失败")
            print("💡 查看日志: docker-compose logs")
            return 1
    
    # 兜底返回（理论上不会到达这里）
    return 0


def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="AI Search MCP - Docker 部署管理工具"
    )
    parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="强制重新部署"
    )
    parser.add_argument(
        "--stop", "-s",
        action="store_true",
        help="停止 Docker 服务"
    )
    
    args = parser.parse_args()
    
    try:
        sys.exit(deploy(force=args.force, stop=args.stop))
    except KeyboardInterrupt:
        print("\n⚠️  用户取消")
        sys.exit(130)
    except Exception as e:
        print(f"\n❌ 错误: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
