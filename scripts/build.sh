#!/usr/bin/env bash
#
# GitNexus Rust-core 本地构建脚本
#
# 用法:
#   ./scripts/build.sh              # release 构建（含 Cangjie 特性）
#   ./scripts/build.sh --debug      # debug 构建
#   ./scripts/build.sh --no-cangjie # 不含 Cangjie 特性的 release 构建
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

# --- 参数解析 ---
PROFILE="--release"
FEATURES="--features tree-sitter-cangjie"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            PROFILE=""
            shift
            ;;
        --no-cangjie)
            FEATURES=""
            shift
            ;;
        --help|-h)
            echo "用法: $0 [--debug] [--no-cangjie]"
            echo ""
            echo "选项:"
            echo "  --debug       构建 debug 版本（默认 release）"
            echo "  --no-cangjie  不包含 Cangjie 语言支持（默认包含）"
            echo ""
            echo "输出二进制位置: target/release/gitnexus-rust-core-cli"
            exit 0
            ;;
        *)
            echo "未知选项: $1（使用 --help 查看帮助）"
            exit 1
            ;;
    esac
done

# --- 前置检查 ---
if ! command -v cargo &> /dev/null; then
    echo "错误: 未找到 cargo。请先安装 Rust 工具链:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "==> GitNexus Rust-core 本地构建"
echo "    项目根目录: $PROJECT_ROOT"
echo "    构建模式:   ${PROFILE:-debug}"
echo "    特性:       ${FEATURES:-(无)}"
echo ""

# --- 构建 ---
echo "==> 开始构建..."
# shellcheck disable=SC2086  # PROFILE/FEATURES 可能为空，需要 word splitting
cargo build $PROFILE $FEATURES -p gitnexus-rust-core-cli

# --- 确定二进制路径 ---
if [[ -n "$PROFILE" ]]; then
    BIN_DIR="$PROJECT_ROOT/target/release"
else
    BIN_DIR="$PROJECT_ROOT/target/debug"
fi
BIN_PATH="$BIN_DIR/gitnexus-rust-core-cli"

echo ""
echo "==> 构建完成"
echo "    二进制: $BIN_PATH"
echo ""

# --- 试用提示 ---
echo "==> 快速试用"
echo ""
echo "  # Rust 项目分析"
echo "  $BIN_PATH analyze --root fixtures/rust/portable-smoke --format json"
echo ""
if [[ -n "$FEATURES" ]]; then
    echo "  # Cangjie 项目分析"
    echo "  $BIN_PATH analyze --root fixtures/cangjie/portable-smoke --format json"
    echo ""
fi
echo "  # 自身分析"
echo "  $BIN_PATH analyze --root . --language auto --format json"
echo ""
echo "  # 质量门检查"
echo "  $BIN_PATH quality --root fixtures/rust/portable-smoke --language rust"
echo ""
echo "  # 更多用法见 README.md"
