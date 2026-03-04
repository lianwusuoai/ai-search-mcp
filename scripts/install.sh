#!/bin/bash
# AI Search MCP - 一键安装脚本（Linux/macOS）

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印函数
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_header() {
    echo ""
    echo "╔════════════════════════════════════════╗"
    echo "║   AI Search MCP - 一键安装脚本         ║"
    echo "╚════════════════════════════════════════╝"
    echo ""
}

# 检查命令是否存在
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# 检查 Python
check_python() {
    print_info "检查 Python 环境..."
    
    if command_exists python3; then
        PYTHON_CMD="python3"
    elif command_exists python; then
        PYTHON_CMD="python"
    else
        print_error "未找到 Python，请先安装 Python 3.8+"
        exit 1
    fi
    
    PYTHON_VERSION=$($PYTHON_CMD --version 2>&1 | awk '{print $2}')
    print_success "Python 版本: $PYTHON_VERSION"
}

# 检查 pip
check_pip() {
    print_info "检查 pip..."
    
    if command_exists pip3; then
        PIP_CMD="pip3"
    elif command_exists pip; then
        PIP_CMD="pip"
    else
        print_error "未找到 pip，请先安装 pip"
        exit 1
    fi
    
    print_success "pip 已安装"
}

# 安装 Python 包
install_package() {
    print_info "安装 ai-search-mcp Python 包..."
    
    if $PIP_CMD install --upgrade ai-search-mcp; then
        print_success "Python 包安装成功"
        
        # 获取版本
        VERSION=$(ai-search-mcp --version 2>&1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
        print_info "已安装版本: $VERSION"
    else
        print_error "Python 包安装失败"
        exit 1
    fi
}

# 检查 Docker
check_docker() {
    print_info "检查 Docker 环境..."
    
    if command_exists docker && docker info >/dev/null 2>&1; then
        print_success "Docker 已运行"
        return 0
    else
        print_warning "Docker 未运行或未安装"
        return 1
    fi
}

# 克隆项目
clone_project() {
    print_info "克隆项目..."
    
    # 默认安装到用户目录
    INSTALL_DIR="$HOME/.ai-search-mcp-project"
    
    if [ -d "$INSTALL_DIR" ]; then
        print_info "项目目录已存在，更新中..."
        cd "$INSTALL_DIR"
        git pull
    else
        if git clone https://github.com/lianwusuoai/ai-search-mcp.git "$INSTALL_DIR"; then
            print_success "项目克隆成功"
            cd "$INSTALL_DIR"
        else
            print_error "项目克隆失败"
            return 1
        fi
    fi
    
    # 设置环境变量
    export AI_SEARCH_MCP_ROOT="$INSTALL_DIR"
    print_info "项目目录: $INSTALL_DIR"
    
    return 0
}

# 部署 Docker
deploy_docker() {
    print_info "部署 Docker 容器..."
    
    if ai-search-mcp-deploy; then
        print_success "Docker 部署成功"
        return 0
    else
        print_error "Docker 部署失败"
        return 1
    fi
}

# 打开配置界面
open_config() {
    print_info "打开配置界面..."
    
    CONFIG_URL="http://localhost:11000/config"
    
    # 尝试打开浏览器
    if command_exists xdg-open; then
        xdg-open "$CONFIG_URL" 2>/dev/null || true
    elif command_exists open; then
        open "$CONFIG_URL" 2>/dev/null || true
    fi
    
    print_success "配置界面: $CONFIG_URL"
}

# 显示下一步
show_next_steps() {
    echo ""
    echo "╔════════════════════════════════════════╗"
    echo "║   安装完成！                           ║"
    echo "╚════════════════════════════════════════╝"
    echo ""
    print_info "下一步操作："
    echo ""
    echo "1. 配置 AI API（浏览器打开）："
    echo "   http://localhost:11000/config"
    echo ""
    echo "2. 配置 MCP 客户端（编辑配置文件）："
    echo "   Kiro IDE: .kiro/settings/mcp.json"
    echo "   Claude Desktop: claude_desktop_config.json"
    echo ""
    echo "   添加以下配置："
    echo '   {'
    echo '     "mcpServers": {'
    echo '       "ai-search": {'
    echo '         "command": "uvx",'
    echo '         "args": ["ai-search-mcp"]'
    echo '       }'
    echo '     }'
    echo '   }'
    echo ""
    echo "3. 重启 MCP 客户端"
    echo ""
    print_info "常用命令："
    echo "  - 查看版本: ai-search-mcp --version"
    echo "  - 验证配置: ai-search-mcp --validate-config"
    echo "  - 重新部署: ai-search-mcp-deploy"
    echo "  - 停止服务: ai-search-mcp-deploy --stop"
    echo ""
}

# 主函数
main() {
    print_header
    
    # 检查环境
    check_python
    check_pip
    
    # 安装 Python 包
    install_package
    
    # 检查 Docker
    if check_docker; then
        # Docker 可用，询问是否部署
        echo ""
        read -p "是否部署 Docker 容器？(Y/n) " -n 1 -r
        echo ""
        
        if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
            if clone_project; then
                if deploy_docker; then
                    # 等待服务启动
                    print_info "等待服务启动..."
                    sleep 3
                    
                    # 打开配置界面
                    open_config
                fi
            fi
        else
            print_info "跳过 Docker 部署"
            print_info "你可以稍后运行以下命令部署："
            echo "  git clone https://github.com/lianwusuoai/ai-search-mcp.git"
            echo "  cd ai-search-mcp"
            echo "  ai-search-mcp-deploy"
        fi
    else
        print_warning "Docker 不可用，跳过 Docker 部署"
        print_info "你可以手动启动 HTTP 服务器："
        echo "  ai-search-mcp --mode http --port 11000"
    fi
    
    # 显示下一步
    show_next_steps
}

# 运行主函数
main
