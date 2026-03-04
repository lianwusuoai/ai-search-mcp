#!/usr/bin/env python3
"""
AI Search MCP - 安装后自动部署脚本
在 pip install 后自动检查并部署 Docker 容器
"""
import os
import sys
import subprocess
import json
from pathlib import Path


def run_command(cmd, check=True, capture=True):
    """执行命令"""
    try:
        if capture:
            result = subprocess.run(
                cmd, shell=True, check=check, 
                capture_output=True, text=True, timeout=30
            )
            return result.returncode == 0, result.stdout.strip()
        else:
            result = subprocess.run(cmd, shell=True, check=check, timeout=30)
            return result.returncode == 0, ""
    except Exception as e:
        return False, str(e)


def check_docker():
    """检查 Docker 是否运行"""
    success, _ = run_command("docker info", check=False)
    return success


def get_installed_version():
    """获取当前安装的版本"""
    success, output = run_command("ai-search-mcp --version", check=False)
    if success and output:
        import re
        match = re.search(r'(\d+\.\d+\.\d+)', output)
        if match:
            return match.group(1)
    return None


def get_docker_version():
    """获取 Docker 容器版本"""
    try:
        import urllib.request
        with urllib.request.urlopen("http://localhost:11000/health", timeout=2) as response:
            data = json.loads(response.read().decode())
            return data.get("version")
    except:
        return None


def find_project_root():
    """查找项目根目录（包含 docker-compose.yml）"""
    # 尝试从环境变量获取
    if "AI_SEARCH_MCP_ROOT" in os.environ:
        root = Path(os.environ["AI_SEARCH_MCP_ROOT"])
        if (root / "docker-compose.yml").exists():
            return root
    
    # 尝试从当前目录向上查找
    current = Path.cwd()
    for _ in range(5):  # 最多向上查找 5 层
        if (current / "docker-compose.yml").exists():
            return current
        current = current.parent
    
    return None


def auto_deploy():
    """自动部署逻辑"""
    print("🔍 AI Search MCP - 安装后检查")
    
    # 检查 Docker
    if not check_docker():
        print("⚠️  Docker 未运行，跳过自动部署")
        print("💡 提示：如需 HTTP 模式，请启动 Docker 后运行：")
        print("   cd <项目目录> && docker-compose up -d")
        return
    
    # 查找项目根目录
    project_root = find_project_root()
    if not project_root:
        print("⚠️  未找到 docker-compose.yml，跳过自动部署")
        print("💡 提示：如需自动部署，请设置环境变量：")
        print("   export AI_SEARCH_MCP_ROOT=/path/to/ai-search-mcp")
        return
    
    print(f"📁 项目目录: {project_root}")
    
    # 获取版本信息
    installed_version = get_installed_version()
    docker_version = get_docker_version()
    
    print(f"📦 已安装版本: {installed_version}")
    print(f"🐳 Docker 版本: {docker_version or '未运行'}")
    
    # 判断是否需要部署
    need_deploy = False
    
    if docker_version is None:
        print("ℹ️  Docker 容器未运行，准备首次部署")
        need_deploy = True
    elif installed_version != docker_version:
        print("⚠️  版本不一致，准备更新 Docker 容器")
        need_deploy = True
    else:
        print("✅ 版本一致，无需更新")
        return
    
    # 执行部署
    if need_deploy:
        print("\n🚀 开始自动部署...")
        os.chdir(project_root)
        
        # 停止旧容器
        print("⏹️  停止旧容器...")
        run_command("docker-compose down", check=False, capture=False)
        
        # 构建并启动
        print(f"🔨 构建并启动新容器（版本: {installed_version}）...")
        success, _ = run_command("docker-compose up -d --build", check=False, capture=False)
        
        if success:
            print("\n✅ 部署成功！")
            print("🌐 服务地址: http://localhost:11000")
            print("⚙️  配置界面: http://localhost:11000/config")
        else:
            print("\n❌ 部署失败，请手动检查")
            print("💡 查看日志: docker-compose logs")


if __name__ == "__main__":
    try:
        auto_deploy()
    except KeyboardInterrupt:
        print("\n⚠️  用户取消")
        sys.exit(0)
    except Exception as e:
        print(f"\n❌ 错误: {e}")
        sys.exit(1)
