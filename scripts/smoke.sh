#!/usr/bin/env bash
#
# GitNexus Rust-core 快速 Smoke 验证脚本
#
# 用法:
#   ./scripts/smoke.sh              # 全部检查
#   ./scripts/smoke.sh --quick      # 仅 CLI smoke（跳过 cargo test）
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

QUICK_MODE=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick) QUICK_MODE=true; shift ;;
        --help|-h)
            echo "用法: $0 [--quick]"
            echo "  --quick  跳过 cargo test，仅做 CLI smoke"
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

echo "============================================"
echo " GitNexus Rust-core Smoke 验证"
echo " 项目根目录: $PROJECT_ROOT"
echo " 时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo "============================================"
echo ""

# --- Step 1: 代码格式 ---
echo "--- Step 1: 代码格式 ---"
if cargo fmt --check 2>/dev/null; then
    pass "cargo fmt --check"
else
    fail "cargo fmt --check（格式不一致，请运行 cargo fmt）"
fi
echo ""

# --- Step 2: 测试 ---
if $QUICK_MODE; then
    skip "cargo test（--quick 模式跳过）"
else
    echo "--- Step 2: 测试（no-feature） ---"
    if cargo test 2>&1 | tail -5; then
        pass "cargo test（no-feature）"
    else
        fail "cargo test（no-feature）"
    fi

    echo ""
    echo "--- Step 3: 测试（feature: tree-sitter-cangjie） ---"
    if cargo test --features tree-sitter-cangjie 2>&1 | tail -5; then
        pass "cargo test（with feature）"
    else
        fail "cargo test（with feature）"
    fi
fi
echo ""

# --- Step 3: Rust CLI smoke ---
echo "--- Step 4: Rust analyze --strict（portable-smoke fixture） ---"
RUST_OUTPUT=$(cargo run -p gitnexus-rust-core-cli -- analyze --root fixtures/rust/portable-smoke --language rust --format json --strict 2>/dev/null) || true
RUST_EXIT=$?
if [[ $RUST_EXIT -eq 0 ]] && echo "$RUST_OUTPUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
s = d['summary']
gates = d['qualityGates']
passed = sum(1 for g in gates if g['passed'])
assert s['nodeCount'] > 0, 'nodeCount is 0'
assert s['edgeCount'] > 0, 'edgeCount is 0'
assert passed == len(gates), f'quality gates: {passed}/{len(gates)}'
print(f'  nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]} symbols={s[\"symbolCount\"]} quality={passed}/{len(gates)}')
" 2>/dev/null; then
    pass "Rust analyze --strict: $(echo "$RUST_OUTPUT" | python3 -c "import json,sys; d=json.load(sys.stdin); s=d['summary']; print(f'nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]}')" 2>/dev/null || echo 'ok')"
else
    fail "Rust analyze --strict（exit=$RUST_EXIT）"
fi

echo ""
echo "--- Step 5: Rust quality（exit code 检查） ---"
if cargo run -p gitnexus-rust-core-cli -- quality --root fixtures/rust/portable-smoke --language rust 2>/dev/null; then
    pass "Rust quality exit code 0 (pass)"
else
    fail "Rust quality exit code 非 0"
fi

echo ""

# --- Step 4: Cangjie CLI smoke ---
echo "--- Step 6: Cangjie analyze --strict（portable-smoke fixture） ---"
CANGJIE_OUTPUT=$(cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze --root fixtures/cangjie/portable-smoke --language cangjie --format json --strict 2>/dev/null) || true
CANGJIE_EXIT=$?
if [[ $CANGJIE_EXIT -eq 0 ]] && echo "$CANGJIE_OUTPUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
s = d['summary']
gates = d['qualityGates']
passed = sum(1 for g in gates if g['passed'])
assert s['nodeCount'] > 0, 'nodeCount is 0'
assert s['edgeCount'] > 0, 'edgeCount is 0'
assert passed == len(gates), f'quality gates: {passed}/{len(gates)}'
print(f'  nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]} symbols={s[\"symbolCount\"]} quality={passed}/{len(gates)}')
" 2>/dev/null; then
    pass "Cangjie analyze --strict: $(echo "$CANGJIE_OUTPUT" | python3 -c "import json,sys; d=json.load(sys.stdin); s=d['summary']; print(f'nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]}')" 2>/dev/null || echo 'ok')"
else
    fail "Cangjie analyze --strict（exit=$CANGJIE_EXIT）"
fi

echo ""
echo "--- Step 7: Cangjie quality（exit code 检查） ---"
if cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- quality --root fixtures/cangjie/portable-smoke --language cangjie 2>/dev/null; then
    pass "Cangjie quality exit code 0 (pass)"
else
    fail "Cangjie quality exit code 非 0"
fi

echo ""

# --- Step 5: 自身分析 ---
echo "--- Step 8: 自身 smoke（analyze --language auto） ---"
if cargo run -p gitnexus-rust-core-cli -- analyze --root . --language auto --format json 2>/dev/null | python3 -c "
import json, sys
d = json.load(sys.stdin)
s = d['summary']
assert s['nodeCount'] > 100, f'self-analysis nodeCount too low: {s[\"nodeCount\"]}'
print(f'  nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]} symbols={s[\"symbolCount\"]}')
" 2>/dev/null; then
    pass "自身 smoke"
else
    fail "自身 smoke"
fi

echo ""

# --- 汇总 ---
TOTAL=$((PASS + FAIL + SKIP))
echo "============================================"
echo " Smoke 结果汇总"
echo "============================================"
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo "  SKIP: $SKIP"
echo "  TOTAL: $TOTAL"
echo ""

if [[ $FAIL -gt 0 ]]; then
    echo "*** 存在失败项，请检查。 ***"
    exit 1
else
    echo "全部通过。"
    exit 0
fi
