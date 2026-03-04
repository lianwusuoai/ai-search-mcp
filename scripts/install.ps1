# AI Search MCP - 一键安装脚本（Windows PowerShell）

# 设置控制台编码为 UTF-8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# 设置错误时停止
$ErrorActionPreference = "Stop"

# 颜色函数
function Write-Info {
    param([string]$Message)
    Write-Host "ℹ️  $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "✅ $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "⚠️  $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "❌ $Message" -ForegroundColor Red
}

function Write-Header {
    Write-Host ""
    Write-Host "╔════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║   AI Search MCP - 一键安装脚本         ║" -ForegroundColor Cyan
    Write-Host "╚════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
}

# 检查命令是否存在
function Test-Command {
    param([string]$Command)
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    } catch {
        return $false
    }
}

# 检查 Python
function Test-Python {
    Write-Info "检查 Python 环境..."
    
    if (Test-Command "python") {
        $pythonVersion = python --version 2>&1
        Write-Success "Python 版本: $pythonVersion"
        return $true
    } else {
        Write-Error "未找到 Python，请先安装 Python 3.8+"
        Write-Info "下载地址: https://www.python.org/downloads/"
        return $false
    }
}

# 检查 pip
function Test-Pip {
    Write-Info "检查 pip..."
    
    if (Test-Command "pip") {
        Write-Success "pip 已安装"
        return $true
    } else {
        Write-Error "未找到 pip，请先安装 pip"
        return $false
    }
}

# 获取当前版本
function Get-CurrentVersion {
    if (Test-Command "ai-search-mcp") {
        try {
            $version = ai-search-mcp --version 2>&1 | Select-String -Pattern '\d+\.\d+\.\d+' | ForEach-Object { $_.Matches[0].Value }
            return $version
        } catch {
            return $null
        }
    }
    return $null
}

# 安装 Python 包
function Install-Package {
    # 检查当前版本
    $currentVersion = Get-CurrentVersion
    if ($currentVersion) {
        Write-Info "当前 MCP 版本: $currentVersion"
    } else {
        Write-Info "首次安装 ai-search-mcp"
    }
    
    Write-Info "安装 ai-search-mcp Python 包..."
    
    try {
        pip install --upgrade ai-search-mcp 2>&1 | Out-Null
        
        # 获取新版本
        $newVersion = ai-search-mcp --version 2>&1 | Select-String -Pattern '\d+\.\d+\.\d+' | ForEach-Object { $_.Matches[0].Value }
        
        if ($currentVersion -and $currentVersion -ne $newVersion) {
            Write-Success "MCP 已升级: $currentVersion → $newVersion"
        } elseif ($currentVersion) {
            Write-Success "MCP 已是最新版本: $newVersion"
        } else {
            Write-Success "MCP 安装成功: $newVersion"
        }
        
        return $true
    } catch {
        Write-Error "Python 包安装失败: $_"
        return $false
    }
}

# 检查 Docker
function Test-Docker {
    Write-Info "检查 Docker 环境..."
    
    if (Test-Command "docker") {
        try {
            # 获取 Docker 版本
            $dockerVersion = docker --version 2>&1 | Select-String -Pattern 'Docker version [\d\.]+' | ForEach-Object { $_.Matches[0].Value }
            
            docker info 2>&1 | Out-Null
            Write-Success "Docker 已运行 ($dockerVersion)"
            return $true
        } catch {
            Write-Warning "Docker 未运行"
            return $false
        }
    } else {
        Write-Warning "Docker 未安装"
        return $false
    }
}

# 克隆项目
function Clone-Project {
    Write-Info "克隆项目..."
    
    # 默认安装到用户目录
    $installDir = "$env:USERPROFILE\.ai-search-mcp"
    
    if (Test-Path $installDir) {
        Write-Info "项目目录已存在，更新中..."
        Set-Location $installDir
        git pull
    } else {
        try {
            git clone https://github.com/lianwusuoai/ai-search-mcp.git $installDir
            Write-Success "项目克隆成功"
            Set-Location $installDir
        } catch {
            Write-Error "项目克隆失败: $_"
            return $false
        }
    }
    
    # 设置环境变量
    $env:AI_SEARCH_MCP_ROOT = $installDir
    Write-Info "项目目录: $installDir"
    
    return $true
}

# 获取 Docker 容器版本
function Get-DockerVersion {
    try {
        $containerExists = docker ps -a --filter "name=ai-search" --format "{{.Names}}" 2>&1
        if ($containerExists -eq "ai-search") {
            $imageId = docker inspect ai-search --format "{{.Image}}" 2>&1
            $imageCreated = docker inspect $imageId --format "{{.Created}}" 2>&1 | Select-String -Pattern '\d{4}-\d{2}-\d{2}' | ForEach-Object { $_.Matches[0].Value }
            return $imageCreated
        }
    } catch {
        return $null
    }
    return $null
}

# 部署 Docker
function Deploy-Docker {
    # 检查当前 Docker 版本
    $currentDockerVersion = Get-DockerVersion
    if ($currentDockerVersion) {
        Write-Info "当前 Docker 镜像: $currentDockerVersion"
    } else {
        Write-Info "首次部署 Docker 容器"
    }
    
    Write-Info "部署 Docker 容器..."
    
    try {
        # 执行部署命令并捕获退出码
        ai-search-mcp-deploy
        
        # 检查退出码
        if ($LASTEXITCODE -eq 0) {
            $newDockerVersion = Get-DockerVersion
            if ($currentDockerVersion -and $currentDockerVersion -ne $newDockerVersion) {
                Write-Success "Docker 已升级: $currentDockerVersion → $newDockerVersion"
            } elseif ($currentDockerVersion) {
                Write-Success "Docker 部署成功（镜像未变化）"
            } else {
                Write-Success "Docker 部署成功: $newDockerVersion"
            }
            return $true
        } else {
            Write-Error "Docker 部署失败（退出码: $LASTEXITCODE）"
            Write-Info "请查看上方错误信息"
            return $false
        }
    } catch {
        Write-Error "Docker 部署失败: $_"
        return $false
    }
}

# 打开配置界面
function Open-Config {
    Write-Info "打开配置界面..."
    
    $configUrl = "http://localhost:11000"
    
    try {
        Start-Process $configUrl
        Write-Success "配置界面: $configUrl"
    } catch {
        Write-Warning "无法自动打开浏览器，请手动访问: $configUrl"
    }
}

# 显示下一步
function Show-NextSteps {
    Write-Host ""
    Write-Host "╔════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║   安装完成！                           ║" -ForegroundColor Cyan
    Write-Host "╚════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
    Write-Info "下一步操作："
    Write-Host ""
    Write-Host "1. 配置 AI API（浏览器打开）："
    Write-Host "   http://localhost:11000"
    Write-Host "   （需要输入管理员密码进入配置页面）"
    Write-Host ""
    Write-Host "2. 配置 MCP 客户端（编辑配置文件）："
    Write-Host "   Kiro IDE: .kiro/settings/mcp.json"
    Write-Host "   Claude Desktop: claude_desktop_config.json"
    Write-Host ""
    Write-Host "   添加以下配置："
    Write-Host '   {'
    Write-Host '     "mcpServers": {'
    Write-Host '       "ai-search": {'
    Write-Host '         "command": "uvx",'
    Write-Host '         "args": ["ai-search-mcp"]'
    Write-Host '       }'
    Write-Host '     }'
    Write-Host '   }'
    Write-Host ""
    Write-Host "3. 重启 MCP 客户端"
    Write-Host ""
    Write-Info "常用命令："
    Write-Host "  - 查看版本: ai-search-mcp --version"
    Write-Host "  - 验证配置: ai-search-mcp --validate-config"
    Write-Host "  - 重新部署: ai-search-mcp-deploy"
    Write-Host "  - 停止服务: ai-search-mcp-deploy --stop"
    Write-Host ""
}

# 主函数
function Main {
    Write-Header
    
    # 检查环境
    if (-not (Test-Python)) { exit 1 }
    if (-not (Test-Pip)) { exit 1 }
    
    # 安装 Python 包
    if (-not (Install-Package)) { exit 1 }
    
    # 检查 Docker
    if (Test-Docker) {
        # Docker 可用，询问是否部署
        Write-Host ""
        $response = Read-Host "是否部署 Docker 容器？(Y/n)"
        
        if ([string]::IsNullOrWhiteSpace($response) -or $response -match '^[Yy]') {
            if (Clone-Project) {
                if (Deploy-Docker) {
                    # 等待服务启动
                    Write-Info "等待服务启动..."
                    Start-Sleep -Seconds 3
                    
                    # 打开配置界面
                    Open-Config
                }
            }
        } else {
            Write-Info "跳过 Docker 部署"
            Write-Info "你可以稍后运行以下命令部署："
            Write-Host "  git clone https://github.com/lianwusuoai/ai-search-mcp.git"
            Write-Host "  cd ai-search-mcp"
            Write-Host "  ai-search-mcp-deploy"
        }
    } else {
        Write-Warning "Docker 不可用，跳过 Docker 部署"
        Write-Info "你可以手动启动 HTTP 服务器："
        Write-Host "  ai-search-mcp --mode http --port 11000"
    }
    
    # 显示下一步
    Show-NextSteps
}

# 运行主函数
try {
    Main
} catch {
    Write-Error "安装过程中发生错误: $_"
    exit 1
}
