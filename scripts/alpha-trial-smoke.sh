#!/usr/bin/env bash
#
# Alpha Trial 端到端 Smoke — 验证 Rust/Cangjie bridge JSON → Tool 导入全链路
#
# 用法:
#   ./scripts/alpha-trial-smoke.sh              # 全部检查
#   ./scripts/alpha-trial-smoke.sh --rust-only  # 仅 Rust
#   ./scripts/alpha-trial-smoke.sh --cangjie-only  # 仅 Cangjie
#   ./scripts/alpha-trial-smoke.sh --help
#
# 约束:
#   - 不新增依赖
#   - 只使用 portable-smoke fixture 或只读 index checkout
#   - 不写 live repo
#   - 不自动 commit
#   - 失败时 exit non-zero
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"
NODE_BIN="${NODE_BIN:-node}"
RESTORE_REPO_NAME="${CODELATTICE_REPO_NAME:-$(basename "$PROJECT_ROOT")}"

RUST_ONLY=false
CANGJIE_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --rust-only) RUST_ONLY=true; shift ;;
        --cangjie-only) CANGJIE_ONLY=true; shift ;;
        --help|-h)
            echo "用法: $0 [--rust-only|--cangjie-only]"
            echo "  --rust-only     仅验证 Rust bridge → Tool 导入"
            echo "  --cangjie-only  仅验证 Cangjie bridge → Tool 导入"
            echo ""
            echo "环境要求:"
            echo "  - cargo (Rust toolchain)"
            echo "  - node (Tool CLI)"
            echo "  - python3 (JSON 验证)"
            exit 0
            ;;
        *) shift ;;
    esac
done

PASS=0
FAIL=0
SKIP=0

pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1"; FAIL=$((FAIL + 1)); }
skip() { echo "  [SKIP] $1"; SKIP=$((SKIP + 1)); }

TMPDIR_SMOKESRC="$(mktemp -d)"
cleanup() {
    rm -rf "$TMPDIR_SMOKESRC"
    # Tool bridge 导入会把当前 repo 临时注册为 alpha-trial-*。
    # smoke 结束时恢复当前项目索引名，避免后续执行 AI 找不到 codelattice。
    if [[ "${CODELATTICE_SKIP_INDEX_RESTORE:-0}" != "1" && -f "$TOOL" && -d "$PROJECT_ROOT/.git" ]]; then
        "$NODE_BIN" "$TOOL" analyze "$PROJECT_ROOT" \
            --force \
            --skip-agents-md \
            --name "$RESTORE_REPO_NAME" \
            >/dev/null 2>&1 || true
    fi
    if [[ -d "$PROJECT_ROOT/.git" ]] && ! git -C "$PROJECT_ROOT" ls-files -- .claude | grep -q .; then
        rm -rf "$PROJECT_ROOT/.claude"
    fi
    if [[ -d "$PROJECT_ROOT/.git" ]] && ! git -C "$PROJECT_ROOT" ls-files -- CLAUDE.md | grep -q .; then
        rm -f "$PROJECT_ROOT/CLAUDE.md"
    fi
}
trap cleanup EXIT

echo "============================================"
echo " Alpha Trial 端到端 Smoke"
echo " 时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo "============================================"
echo ""

# --- 检查前置条件 ---

echo "--- 前置条件检查 ---"

if ! command -v python3 &>/dev/null; then
    fail "python3 未安装"
    echo ""
    echo "============================================"
    echo " PASS: $PASS  FAIL: $FAIL  SKIP: $SKIP"
    echo "============================================"
    exit 1
fi
pass "python3 可用"

if [[ ! -f "$TOOL" ]]; then
    fail "Tool CLI 不存在: $TOOL"
    echo ""
    echo "============================================"
    echo " PASS: $PASS  FAIL: $FAIL  SKIP: $SKIP"
    echo "============================================"
    exit 1
fi
pass "Tool CLI 存在"

echo ""

# --- Rust Bridge ---

if [[ "$CANGJIE_ONLY" == "true" ]]; then
    skip "Rust bridge（--cangjie-only）"
else
    echo "--- Rust Bridge → Tool 导入 ---"

RUST_FIXTURE="$PROJECT_ROOT/fixtures/rust/portable-smoke"
RUST_BRIDGE_JSON="$TMPDIR_SMOKESRC/rust-bridge.json"

if [[ ! -d "$RUST_FIXTURE" ]]; then
    fail "Rust portable-smoke fixture 不存在"
else
    # 生成 bridge JSON
    cd "$PROJECT_ROOT"
    if cargo run -p gitnexus-rust-core-cli -- analyze \
        --root "$RUST_FIXTURE" \
        --language rust \
        --format gitnexus-rc \
        --strict \
        > "$RUST_BRIDGE_JSON" 2>/dev/null; then
        pass "Rust bridge JSON 生成成功"
    else
        fail "Rust bridge JSON 生成失败"
    fi

    # 验证 stdout 纯净
    if python3 -c "import json; json.load(open('$RUST_BRIDGE_JSON'))" 2>/dev/null; then
        pass "Rust bridge JSON.parse 成功"
    else
        fail "Rust bridge JSON.parse 失败 — stdout 不纯净"
    fi

    # Tool 导入
    if "$NODE_BIN" "$TOOL" analyze \
        --force \
        --experimental-rust-core-bridge-graph "$RUST_BRIDGE_JSON" \
        --name alpha-trial-rust-smoke \
        --skip-agents-md \
        2>/dev/null | grep -q "indexed successfully"; then
        pass "Rust bridge → Tool 导入成功"
    else
        # 可能 name 冲突，用 --allow-duplicate-name 重试
        if "$NODE_BIN" "$TOOL" analyze \
            --force \
            --allow-duplicate-name \
            --experimental-rust-core-bridge-graph "$RUST_BRIDGE_JSON" \
            --name alpha-trial-rust-smoke \
            --skip-agents-md \
            2>/dev/null | grep -q "indexed successfully"; then
            pass "Rust bridge → Tool 导入成功（duplicate-name 允许）"
        else
            fail "Rust bridge → Tool 导入失败"
        fi
    fi
    fi
fi

echo ""

# --- Cangjie Bridge ---

if [[ "$RUST_ONLY" == "true" ]]; then
    skip "Cangjie bridge（--rust-only）"
else
    echo "--- Cangjie Bridge → Tool 导入 ---"

    CANGJIE_FIXTURE="$PROJECT_ROOT/fixtures/cangjie/portable-smoke"
    CANGJIE_BRIDGE_JSON="$TMPDIR_SMOKESRC/cangjie-bridge.json"

    if [[ ! -d "$CANGJIE_FIXTURE" ]]; then
        fail "Cangjie portable-smoke fixture 不存在"
    else
        # 生成 bridge JSON
        cd "$PROJECT_ROOT"
        if cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
            --root "$CANGJIE_FIXTURE" \
            --language cangjie \
            --format gitnexus-rc \
            --strict \
            > "$CANGJIE_BRIDGE_JSON" 2>/dev/null; then
            pass "Cangjie bridge JSON 生成成功"
        else
            fail "Cangjie bridge JSON 生成失败"
        fi

        # 验证 stdout 纯净
        if python3 -c "import json; json.load(open('$CANGJIE_BRIDGE_JSON'))" 2>/dev/null; then
            pass "Cangjie bridge JSON.parse 成功"
        else
            fail "Cangjie bridge JSON.parse 失败 — stdout 不纯净"
        fi

        # Tool 导入
        if "$NODE_BIN" "$TOOL" analyze \
            --force \
            --experimental-rust-core-bridge-graph "$CANGJIE_BRIDGE_JSON" \
            --name alpha-trial-cangjie-smoke \
            --skip-agents-md \
            2>/dev/null | grep -q "indexed successfully"; then
            pass "Cangjie bridge → Tool 导入成功"
        else
            if "$NODE_BIN" "$TOOL" analyze \
                --force \
                --allow-duplicate-name \
                --experimental-rust-core-bridge-graph "$CANGJIE_BRIDGE_JSON" \
                --name alpha-trial-cangjie-smoke \
                --skip-agents-md \
                2>/dev/null | grep -q "indexed successfully"; then
                pass "Cangjie bridge → Tool 导入成功（duplicate-name 允许）"
            else
                fail "Cangjie bridge → Tool 导入失败"
            fi
        fi
    fi
fi

echo ""

# --- 总结 ---

echo "============================================"
echo " Alpha Trial 端到端 Smoke 结果"
echo "============================================"
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo "  SKIP: $SKIP"
echo ""

if [[ $FAIL -gt 0 ]]; then
    echo "存在失败项 — Alpha Trial 不可用。"
    exit 1
else
    echo "全部通过 — Alpha Trial 端到端链路正常。"
    exit 0
fi
