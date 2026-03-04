# 多阶段构建：编译阶段（使用 nightly 支持 edition2024）
FROM rustlang/rust:nightly-bookworm-slim AS builder

WORKDIR /build

# 安装编译依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制项目文件
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web

# 编译发布版本
RUN cargo build --release

# 运行阶段：最小化镜像（使用相同的 Debian 版本）
FROM debian:bookworm-slim

WORKDIR /app

# 设置时区为北京时间
ENV TZ=Asia/Shanghai

# 安装运行时依赖（curl 用于健康检查，tzdata 用于时区）
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    tzdata \
    && ln -snf /usr/share/zoneinfo/$TZ /etc/localtime \
    && echo $TZ > /etc/timezone \
    && rm -rf /var/lib/apt/lists/*

# 从构建阶段复制二进制文件
COPY --from=builder /build/target/release/ai-search-mcp /app/ai-search-mcp

# 暴露 HTTP 端口
EXPOSE 11000

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:11000/health || exit 1

# 默认以 HTTP 模式运行
ENTRYPOINT ["/app/ai-search-mcp"]
CMD ["--mode", "http", "--port", "11000"]
