#!/usr/bin/env pwsh
# AI Search MCP - 自动部署脚本
# 功能：检测版本变化，自动更新 Docker 容器

param(
    [switch]$Force,  # 强制重新部署
    [switch]$Stop    # 停止 Docker 服务
)

$ErrorActionPreference = "Stop"

# 颜色输出函数
function Write-Info { Write-Host "ℹ️  $args" -ForegroundColor Cyan }
function Write-Success { Write-Host "✅ $args" -ForegroundColor Green }
function Write-Warning { Write-Host "⚠️  $args" -ForegroundColor Yellow }
function Write-Error { Write-Host "❌ $args" -ForegroundColor Red }

# 检查 Docker 是否运行
function Test-DockerRunning {
    try {
        docker info | Out-Null
        return $true
    } catch {
        return $false
    }
}

# 获取当前安装的版本
function Get-InstalledVersion {
    try {
        $version = ai-search-mcp --version 2>&1 | Select-String -Pattern "(\d+\.\d+\.\d+)" | ForEach-Object { $_.Matches.Groups[1].Value }
        return $version
    } catch {
        return $null
    }
}

# 获取 Docker 容器版本
function Get-DockerVersion {
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:11000/health" -TimeoutSec 2 -ErrorAction SilentlyContinue
        return $response.version
    } catch {
        return $null
    }
}

# 主逻辑
Write-Info "AI Search MCP - 自动部署检查"
Write-Host ""

# 检查 Docker
if (-not (Test-DockerRunning)) {
    Write-Error "Docker 未运行，请先启动 Docker Desktop"
    exit 1
}

# 停止服务
if ($Stop) {
    Write-Info "停止 Docker 服务..."
    docker-compose down
    Write-Success "服务已停止"
    exit 0
}

# 获取版本信息
$installedVersion = Get-InstalledVersion
$dockerVersion = Get-DockerVersion

Write-Info "已安装版本: $installedVersion"
Write-Info "Docker 版本: $dockerVersion"
Write-Host ""

# 判断是否需要部署
$needDeploy = $false

if ($Force) {
    Write-Warning "强制重新部署模式"
    $needDeploy = $true
} elseif ($null -eq $dockerVersion) {
    Write-Info "Docker 容器未运行，需要部署"
    $needDeploy = $true
} elseif ($installedVersion -ne $dockerVersion) {
    Write-Warning "版本不一致，需要更新 Docker 容器"
    $needDeploy = $true
} else {
    Write-Success "版本一致，无需更新"
    Write-Info "Docker 容器正常运行: http://localhost:11000"
    exit 0
}

# 执行部署
if ($needDeploy) {
    Write-Info "开始部署..."
    Write-Host ""
    
    # 停止旧容器
    Write-Info "停止旧容器..."
    docker-compose down 2>&1 | Out-Null
    
    # 重新构建并启动
    Write-Info "构建并启动新容器（版本: $installedVersion）..."
    docker-compose up -d --build
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Success "部署成功！"
        Write-Info "服务地址: http://localhost:11000"
        Write-Info "配置界面: http://localhost:11000/config"
        Write-Info "健康检查: http://localhost:11000/health"
    } else {
        Write-Error "部署失败，请检查日志: docker-compose logs"
        exit 1
    }
}
