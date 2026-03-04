# 多阶段构建：编译阶段（使用 nightly 支持 edition2024）
FROM rustlang/rust:nightly-bookworm-slim AS builder

WORKDIR /build

# 配置 Debian 国内镜像源（阿里云）
RUN sed -i 's/deb.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources && \
    sed -i 's/security.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources

# 安装编译依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 配置 Cargo 国内镜像源（字节跳动）
RUN mkdir -p /usr/local/cargo && \
    echo '[source.crates-io]' > /usr/local/cargo/config.toml && \
    echo 'replace-with = "rsproxy-sparse"' >> /usr/local/cargo/config.toml && \
    echo '[source.rsproxy]' >> /usr/local/cargo/config.toml && \
    echo 'registry = "https://rsproxy.cn/crates.io-index"' >> /usr/local/cargo/config.toml && \
    echo '[source.rsproxy-sparse]' >> /usr/local/cargo/config.toml && \
    echo 'registry = "sparse+https://rsproxy.cn/index/"' >> /usr/local/cargo/config.toml && \
    echo '[registries.rsproxy]' >> /usr/local/cargo/config.toml && \
    echo 'index = "https://rsproxy.cn/crates.io-index"' >> /usr/local/cargo/config.toml && \
    echo '[net]' >> /usr/local/cargo/config.toml && \
    echo 'git-fetch-with-cli = true' >> /usr/local/cargo/config.toml

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

# 配置 Debian 国内镜像源（阿里云）
RUN sed -i 's/deb.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources && \
    sed -i 's/security.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources

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

# 复制示例配置文件
COPY config.example.json /app/config.example.json

# 创建启动脚本
RUN echo '#!/bin/bash' > /app/entrypoint.sh && \
    echo 'set -e' >> /app/entrypoint.sh && \
    echo '' >> /app/entrypoint.sh && \
    echo '# 如果配置文件不存在，复制示例配置' >> /app/entrypoint.sh && \
    echo 'if [ ! -f /root/.ai-search-mcp/config.json ]; then' >> /app/entrypoint.sh && \
    echo '  echo "配置文件不存在，创建示例配置..."' >> /app/entrypoint.sh && \
    echo '  mkdir -p /root/.ai-search-mcp' >> /app/entrypoint.sh && \
    echo '  cp /app/config.example.json /root/.ai-search-mcp/config.json' >> /app/entrypoint.sh && \
    echo '  echo "示例配置已创建，请访问 http://localhost:11000/config 进行配置"' >> /app/entrypoint.sh && \
    echo 'fi' >> /app/entrypoint.sh && \
    echo '' >> /app/entrypoint.sh && \
    echo '# 启动应用' >> /app/entrypoint.sh && \
    echo 'exec /app/ai-search-mcp "$@"' >> /app/entrypoint.sh && \
    chmod +x /app/entrypoint.sh

# 暴露 HTTP 端口
EXPOSE 11000

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:11000/health || exit 1

# 使用启动脚本
ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["--mode", "http", "--port", "11000"]
