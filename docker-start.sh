#!/bin/bash
# AI Search MCP Server - Docker 快速启动脚本

set -e

echo "=== AI Search MCP Server - Docker 启动 ==="

# 检查 Docker 是否安装
echo -e "\n[1/4] 检查 Docker 环境..."
if ! command -v docker &> /dev/null; then
    echo "✗ 错误: 未检测到 Docker"
    echo "请先安装 Docker: https://docs.docker.com/get-docker/"
    exit 1
fi
echo "✓ Docker 已安装: $(docker --version)"

if ! command -v docker-compose &> /dev/null; then
    echo "✗ 错误: 未检测到 Docker Compose"
    exit 1
fi
echo "✓ Docker Compose 已安装"

# 检查配置文件
echo -e "\n[2/4] 检查配置文件..."
config_path="$HOME/.ai-search-mcp/config.json"

if [ ! -f "$config_path" ]; then
    echo "✗ 未找到配置文件: $config_path"
    echo -e "\n请先配置 MCP 服务器："
    echo "  1. 启动 HTTP 服务器: ai-search-mcp --mode http --port 11000"
    echo "  2. 浏览器打开: http://localhost:11000/config"
    echo "  3. 填写配置并保存"
    echo "  4. 重新运行此脚本"
    exit 1
fi

echo "✓ 配置文件存在: $config_path"

# 检查端口占用
echo -e "\n[3/4] 检查端口占用..."
port=11000

if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "⚠ 端口 $port 已被占用"
    pid=$(lsof -Pi :$port -sTCP:LISTEN -t)
    echo "占用进程 PID: $pid"
    
    read -p "是否停止现有服务并重启？(y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "正在停止现有服务..."
        docker-compose down
        sleep 2
    else
        echo "已取消启动"
        exit 0
    fi
else
    echo "✓ 端口 $port 可用"
fi

# 启动服务
echo -e "\n[4/4] 启动 Docker 服务..."
echo "执行: docker-compose up -d --build"

docker-compose up -d --build

if [ $? -eq 0 ]; then
    echo -e "\n✓ 服务启动成功！"
    echo -e "\n服务信息："
    echo "  - 健康检查: http://localhost:$port/health"
    echo "  - 配置界面: http://localhost:$port/config"
    echo "  - API 端点: http://localhost:$port/api/search"
    echo "  - SSE 端点: http://localhost:$port/sse"
    
    echo -e "\n常用命令："
    echo "  - 查看日志: docker-compose logs -f"
    echo "  - 停止服务: docker-compose stop"
    echo "  - 重启服务: docker-compose restart"
    echo "  - 删除服务: docker-compose down"
    
    # 等待服务就绪
    echo -e "\n等待服务就绪..."
    max_retries=10
    retry_count=0
    service_ready=false
    
    while [ $retry_count -lt $max_retries ]; do
        if curl -f -s http://localhost:$port/health > /dev/null 2>&1; then
            service_ready=true
            break
        fi
        retry_count=$((retry_count + 1))
        sleep 1
    done
    
    if [ "$service_ready" = true ]; then
        echo "✓ 服务已就绪！"
        echo -e "\n配置文件: $config_path"
        echo "修改配置: http://localhost:$port/config"
    else
        echo "⚠ 服务启动超时，请检查日志: docker-compose logs"
    fi
else
    echo -e "\n✗ 服务启动失败"
    echo "请查看错误信息或运行: docker-compose logs"
    exit 1
fi
