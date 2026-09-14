# syntax=docker/dockerfile:1
# CodeLattice MCP server 容器镜像。
#
# 用途：
# 1. MCP 注册表（Glama 等）沙箱构建与健康检查——容器入口即 stdio MCP
#    server（`codelattice mcp`），可响应 initialize/introspection。
# 2. 自托管容器部署的基础镜像。
#
# 构建特性与 scripts/package-release.sh 保持一致（全语言适配）。

FROM rust:1-bookworm AS build
WORKDIR /build

# 仅拷贝构建所需文件（配合 .dockerignore 缩小上下文）
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p gitnexus-rust-core-cli \
    --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python \
    --bins

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/target/release/codelattice /usr/local/bin/codelattice
COPY --from=build /build/target/release/gitnexus-rust-core-cli /usr/local/bin/gitnexus-rust-core-cli

ENV LANG=C.UTF-8

# stdio MCP server：默认入口
ENTRYPOINT ["codelattice"]
CMD ["mcp"]
