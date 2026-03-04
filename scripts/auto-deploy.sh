#!/bin/bash
# AI Search MCP - 自动部署脚本
# 功能：检测版本变化，自动更新 Docker 容器

set -e

# 颜色输出
info() { echo -e "\033[36mℹ️  $*\033[0m"; }
success() { echo -e "\033[32m✅ $*\033[0m"; }
warning() { echo -e "\033[33m⚠️  $*\033[0m"; }
error() { echo -e "\033[31m❌ $*\033[0m"; }

# 检查 Docker 是否运行
check_docker() {
    if ! docker info &>/dev/null; then
        error "Docker 未运行，请先启动 Docker"
        exit 1
    fi
}

# 获取当前安装的版本
get_installed_version() {
    ai-search-mcp --version 2>&1 | grep -oP '\d+\.\d+\.\d+' || echo ""
}

# 获取 Docker 容器版本
get_docker_version() {
    curl -s http://localhost:11000/health 2>/dev/null | grep -oP '"version":"\K[^"]+' || echo ""
}

# 解析参数
FORCE=false
STOP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --force) FORCE=true; shift ;;
        --stop) STOP=true; shift ;;
        *) echo "未知参数: $1"; exit 1 ;;
    esac
done

# 主逻辑
info "AI Search MCP - 自动部署检查"
echo ""

check_docker

# 停止服务
if [ "$STOP" = true ]; then
    info "停止 Docker 服务..."
    docker-compose down
    success "服务已停止"
    exit 0
fi

# 获取版本信息
INSTALLED_VERSION=$(get_installed_version)
DOCKER_VERSION=$(get_docker_version)

info "已安装版本: $INSTALLED_VERSION"
info "Docker 版本: $DOCKER_VERSION"
echo ""

# 判断是否需要部署
NEED_DEPLOY=false

if [ "$FORCE" = true ]; then
    warning "强制重新部署模式"
    NEED_DEPLOY=true
elif [ -z "$DOCKER_VERSION" ]; then
    info "Docker 容器未运行，需要部署"
    NEED_DEPLOY=true
elif [ "$INSTALLED_VERSION" != "$DOCKER_VERSION" ]; then
    warning "版本不一致，需要更新 Docker 容器"
    NEED_DEPLOY=true
else
    success "版本一致，无需更新"
    info "Docker 容器正常运行: http://localhost:11000"
    exit 0
fi

# 执行部署
if [ "$NEED_DEPLOY" = true ]; then
    info "开始部署..."
    echo ""
    
    # 停止旧容器
    info "停止旧容器..."
    docker-compose down 2>&1 >/dev/null || true
    
    # 重新构建并启动
    info "构建并启动新容器（版本: $INSTALLED_VERSION）..."
    docker-compose up -d --build
    
    if [ $? -eq 0 ]; then
        echo ""
        success "部署成功！"
        info "服务地址: http://localhost:11000"
        info "配置界面: http://localhost:11000/config"
        info "健康检查: http://localhost:11000/health"
    else
        error "部署失败，请检查日志: docker-compose logs"
        exit 1
    fi
fi
