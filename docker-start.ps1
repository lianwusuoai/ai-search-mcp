#!/usr/bin/env pwsh
# AI Search MCP Server - Docker 快速启动脚本

$ErrorActionPreference = "Stop"

Write-Host "=== AI Search MCP Server - Docker 启动 ===" -ForegroundColor Cyan

# 检查 Docker 是否安装
Write-Host "`n[1/4] 检查 Docker 环境..." -ForegroundColor Yellow
try {
    $dockerVersion = docker --version
    Write-Host "✓ Docker 已安装: $dockerVersion" -ForegroundColor Green
} catch {
    Write-Host "✗ 错误: 未检测到 Docker" -ForegroundColor Red
    Write-Host "请先安装 Docker Desktop: https://www.docker.com/products/docker-desktop/" -ForegroundColor Yellow
    exit 1
}

try {
    docker-compose --version | Out-Null
    Write-Host "✓ Docker Compose 已安装" -ForegroundColor Green
} catch {
    Write-Host "✗ 错误: 未检测到 Docker Compose" -ForegroundColor Red
    exit 1
}

# 检查配置文件
Write-Host "`n[2/4] 检查配置文件..." -ForegroundColor Yellow
$configPath = "$env:USERPROFILE\.ai-search-mcp\config.json"

if (-not (Test-Path $configPath)) {
    Write-Host "✗ 未找到配置文件: $configPath" -ForegroundColor Red
    Write-Host "`n请先配置 MCP 服务器：" -ForegroundColor Yellow
    Write-Host "  1. 启动 HTTP 服务器: ai-search-mcp --mode http --port 11000" -ForegroundColor Cyan
    Write-Host "  2. 浏览器打开: http://localhost:11000/config" -ForegroundColor Cyan
    Write-Host "  3. 填写配置并保存" -ForegroundColor Cyan
    Write-Host "  4. 重新运行此脚本" -ForegroundColor Cyan
    exit 1
}

Write-Host "✓ 配置文件存在: $configPath" -ForegroundColor Green

# 检查端口占用
Write-Host "`n[3/4] 检查端口占用..." -ForegroundColor Yellow
$port = 11000
$portInUse = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue

if ($portInUse) {
    Write-Host "⚠ 端口 $port 已被占用" -ForegroundColor Yellow
    $process = Get-Process -Id $portInUse.OwningProcess -ErrorAction SilentlyContinue
    if ($process) {
        Write-Host "占用进程: $($process.ProcessName) (PID: $($process.Id))" -ForegroundColor Cyan
    }
    
    $response = Read-Host "是否停止现有服务并重启？(y/N)"
    if ($response -eq "y" -or $response -eq "Y") {
        Write-Host "正在停止现有服务..." -ForegroundColor Yellow
        docker-compose down
        Start-Sleep -Seconds 2
    } else {
        Write-Host "已取消启动" -ForegroundColor Yellow
        exit 0
    }
} else {
    Write-Host "✓ 端口 $port 可用" -ForegroundColor Green
}

# 启动服务
Write-Host "`n[4/4] 启动 Docker 服务..." -ForegroundColor Yellow
Write-Host "执行: docker-compose up -d --build" -ForegroundColor Cyan

docker-compose up -d --build

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✓ 服务启动成功！" -ForegroundColor Green
    Write-Host "`n服务信息：" -ForegroundColor Cyan
    Write-Host "  - 健康检查: http://localhost:$port/health" -ForegroundColor White
    Write-Host "  - 配置界面: http://localhost:$port/config" -ForegroundColor White
    Write-Host "  - API 端点: http://localhost:$port/api/search" -ForegroundColor White
    Write-Host "  - SSE 端点: http://localhost:$port/sse" -ForegroundColor White
    
    Write-Host "`n常用命令：" -ForegroundColor Cyan
    Write-Host "  - 查看日志: docker-compose logs -f" -ForegroundColor White
    Write-Host "  - 停止服务: docker-compose stop" -ForegroundColor White
    Write-Host "  - 重启服务: docker-compose restart" -ForegroundColor White
    Write-Host "  - 删除服务: docker-compose down" -ForegroundColor White
    
    # 等待服务就绪
    Write-Host "`n等待服务就绪..." -ForegroundColor Yellow
    $maxRetries = 10
    $retryCount = 0
    $serviceReady = $false
    
    while ($retryCount -lt $maxRetries) {
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:$port/health" -TimeoutSec 2 -ErrorAction Stop
            if ($response.StatusCode -eq 200) {
                $serviceReady = $true
                break
            }
        } catch {
            # 忽略错误，继续重试
        }
        $retryCount++
        Start-Sleep -Seconds 1
    }
    
    if ($serviceReady) {
        Write-Host "✓ 服务已就绪！" -ForegroundColor Green
        Write-Host "`n配置文件: $configPath" -ForegroundColor Gray
        Write-Host "修改配置: http://localhost:$port/config" -ForegroundColor Gray
    } else {
        Write-Host "⚠ 服务启动超时，请检查日志: docker-compose logs" -ForegroundColor Yellow
    }
} else {
    Write-Host "`n✗ 服务启动失败" -ForegroundColor Red
    Write-Host "请查看错误信息或运行: docker-compose logs" -ForegroundColor Yellow
    exit 1
}
