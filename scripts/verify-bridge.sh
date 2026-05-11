#!/usr/bin/env bash
#
# Bridge Format 验证脚本 — 面向下游消费方（adapter / AI workflow / 脚本）开发者
#
# 验证 Rust/Cangjie bridge JSON 输出的：
# 1. 结构完整性（顶层字段 + 端点归一化）
# 2. 端点完整性（无 dangling source/target）
# 3. 统计一致性（stats 与数组计数一致）
# 4. symbol kind 具体化（非通用 "symbol"）
# 5. edge confidence/reason 字段存在性
# 6. packageId 交叉引用一致性
# 7. 输出确定性
#
# 用法:
#   ./scripts/verify-bridge.sh              # 全部检查（需 Cangjie feature）
#   ./scripts/verify-bridge.sh --rust-only  # 仅 Rust bridge
#   ./scripts/verify-bridge.sh --help       # 帮助
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

RUST_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --rust-only) RUST_ONLY=true; shift ;;
        --help|-h)
            echo "用法: $0 [--rust-only]"
            echo "  --rust-only  仅验证 Rust bridge（跳过 Cangjie）"
            exit 0
            ;;
        *) shift ;;
    esac
done

PASS=0
FAIL=0

pass() { echo "  [PASS] $1"; PASS=$((PASS + 1)); }
fail() { echo "  [FAIL] $1"; FAIL=$((FAIL + 1)); }

BIN_CMD="cargo run -p gitnexus-rust-core-cli --bin codelattice --"
BIN_CMD_CJ="cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bin codelattice --"
echo "==> 使用 cargo run 运行 CLI"

echo "============================================"
echo " Bridge Format 验证"
echo " 二进制: cargo run"
echo " 项目根目录: $PROJECT_ROOT"
echo " 时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo "============================================"
echo ""

# ============================================================
# Rust Bridge 验证
# ============================================================
echo "--- Rust Bridge ---"

RUST_BRIDGE=$($BIN_CMD analyze --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc --strict 2>/dev/null) || true
RUST_EXIT=$?

if [[ $RUST_EXIT -ne 0 ]]; then
    fail "Rust bridge: 进程退出非零 (exit=$RUST_EXIT)"
else
    echo "$RUST_BRIDGE" | python3 -c "
import json, sys
d = json.load(sys.stdin)
issues = []

# 1. 结构完整性
required_top = ['schemaVersion','generatedAt','language','root','repository','packages','sourceFiles','symbols','edges','diagnostics','stats']
for f in required_top:
    if f not in d:
        issues.append(f'missing top-level field: {f}')

# repository
if not d.get('repository',{}).get('id'):
    issues.append('repository.id is empty')

# 2. 收集 node-like IDs
ids = set()
if d.get('repository'): ids.add(d['repository']['id'])
for sf in d.get('sourceFiles', []): ids.add(sf['id'])
for sym in d.get('symbols', []): ids.add(sym['id'])
for pkg in d.get('packages', []): ids.add(pkg['id'])

# 3. 端点完整性
edge_cats = ['calls','defines','uses','accesses','designations','imports','contains','owns','annotates','other']
dangling_src = 0
dangling_tgt = 0
total_edges = 0
for cat in edge_cats:
    for edge in d['edges'].get(cat, []):
        total_edges += 1
        if edge['sourceId'] not in ids:
            dangling_src += 1
            if dangling_src <= 3: issues.append(f'dangling source: {edge[\"sourceId\"]}')
        if edge['targetId'] not in ids:
            dangling_tgt += 1
            if dangling_tgt <= 3: issues.append(f'dangling target: {edge[\"targetId\"]}')

# 4. 端点字段名归一化
for cat in edge_cats:
    for edge in d['edges'].get(cat, []):
        if 'source' in edge:
            issues.append(f'{cat} edge has legacy field \"source\" (should be sourceId)')
        if 'target' in edge:
            issues.append(f'{cat} edge has legacy field \"target\" (should be targetId)')

# 5. 统计一致性
stats = d.get('stats', {})
if stats.get('symbolCount', 0) != len(d.get('symbols', [])):
    issues.append('stats.symbolCount mismatch')
if stats.get('sourceFileCount', 0) != len(d.get('sourceFiles', [])):
    issues.append('stats.sourceFileCount mismatch')
if stats.get('packageCount', 0) != len(d.get('packages', [])):
    issues.append('stats.packageCount mismatch')
actual_edge_total = sum(len(d['edges'].get(cat, [])) for cat in edge_cats)
if stats.get('edgeCount', 0) != actual_edge_total:
    issues.append(f'stats.edgeCount mismatch: {stats.get(\"edgeCount\")} vs {actual_edge_total}')

# 6. symbol kind 具体化
for i, sym in enumerate(d.get('symbols', [])):
    if sym.get('kind') == 'symbol':
        issues.append(f'symbol[{i}] ({sym.get(\"name\")}) kind is generic \"symbol\"')

# 7. edge confidence/reason（Rust 语义边要求 confidence 非 null）
semantic_edge_types = ['CALLS', 'ACCESSES', 'DESIGNATION']
for cat in edge_cats:
    for i, edge in enumerate(d['edges'].get(cat, [])):
        if edge.get('kind') in semantic_edge_types and edge.get('confidence') is None:
            issues.append(f'Rust {cat}[{i}] ({edge.get(\"kind\")}) missing confidence')

# 8. packageId 一致性
pkg_ids = {p['id'] for p in d.get('packages', [])}
for i, sf in enumerate(d.get('sourceFiles', [])):
    pid = sf.get('packageId')
    if pid and pid not in pkg_ids:
        issues.append(f'sourceFile[{i}] packageId={pid} not in packages')

if issues:
    for issue in issues:
        print(f'  ISSUE: {issue}')
    sys.exit(1)
else:
    print(f'  OK: {len(d[\"sourceFiles\"])} files, {len(d[\"symbols\"])} symbols, {total_edges} edges, 0 dangling, stats consistent')
" 2>/dev/null

    if [[ $? -eq 0 ]]; then
        pass "Rust bridge: 结构 + 端点 + stats + kind + confidence + packageId"
    else
        fail "Rust bridge: 验证发现问题"
    fi
fi

# Rust 确定性（排除 generatedAt 时间戳差异）
RUST_BRIDGE2=$($BIN_CMD analyze --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc 2>/dev/null) || true
RUST_CMP1=$(echo "$RUST_BRIDGE" | python3 -c "import json,sys; d=json.load(sys.stdin); del d['generatedAt']; print(json.dumps(d, sort_keys=True))" 2>/dev/null)
RUST_CMP2=$(echo "$RUST_BRIDGE2" | python3 -c "import json,sys; d=json.load(sys.stdin); del d['generatedAt']; print(json.dumps(d, sort_keys=True))" 2>/dev/null)
if [[ "$RUST_CMP1" == "$RUST_CMP2" ]]; then
    pass "Rust bridge: 输出确定性（排除时间戳）"
else
    fail "Rust bridge: 两次运行输出不一致"
fi

echo ""

# ============================================================
# Cangjie Bridge 验证
# ============================================================
if $RUST_ONLY; then
    echo "--- Cangjie Bridge: 跳过（--rust-only） ---"
else
    echo "--- Cangjie Bridge ---"

    CJ_BRIDGE=$($BIN_CMD_CJ analyze --root fixtures/cangjie/portable-smoke --language cangjie --format gitnexus-rc --strict 2>/dev/null) || true
    CJ_EXIT=$?

    if [[ $CJ_EXIT -ne 0 ]]; then
        fail "Cangjie bridge: 进程退出非零 (exit=$CJ_EXIT)"
    else
        echo "$CJ_BRIDGE" | python3 -c "
import json, sys
d = json.load(sys.stdin)
issues = []

# 1. 结构完整性
required_top = ['schemaVersion','generatedAt','language','root','repository','packages','sourceFiles','symbols','edges','diagnostics','stats']
for f in required_top:
    if f not in d:
        issues.append(f'missing top-level field: {f}')

ids = set()
if d.get('repository'): ids.add(d['repository']['id'])
for sf in d.get('sourceFiles', []): ids.add(sf['id'])
for sym in d.get('symbols', []): ids.add(sym['id'])
for pkg in d.get('packages', []): ids.add(pkg['id'])

edge_cats = ['calls','defines','uses','accesses','designations','imports','contains','owns','annotates','other']
dangling_src = 0
total_edges = 0
for cat in edge_cats:
    for edge in d['edges'].get(cat, []):
        total_edges += 1
        if edge['sourceId'] not in ids:
            dangling_src += 1
            if dangling_src <= 3: issues.append(f'dangling source: {edge[\"sourceId\"]}')
        if edge['targetId'] not in ids:
            if dangling_src <= 3: issues.append(f'dangling target: {edge[\"targetId\"]}')

# 端点字段名检查
for cat in edge_cats:
    for edge in d['edges'].get(cat, []):
        if 'source' in edge:
            issues.append(f'{cat} edge has legacy field \"source\"')
        if 'target' in edge:
            issues.append(f'{cat} edge has legacy field \"target\"')

# stats 一致性
stats = d.get('stats', {})
if stats.get('symbolCount', 0) != len(d.get('symbols', [])):
    issues.append('stats.symbolCount mismatch')
if stats.get('sourceFileCount', 0) != len(d.get('sourceFiles', [])):
    issues.append('stats.sourceFileCount mismatch')

# symbol kind 具体化
for i, sym in enumerate(d.get('symbols', [])):
    if sym.get('kind') == 'symbol':
        issues.append(f'symbol[{i}] kind is generic \"symbol\"')

# Cangjie 不要求 confidence（源数据不提供），但 edge 结构应完整
for cat in edge_cats:
    for edge in d['edges'].get(cat, []):
        if not edge.get('sourceId'):
            issues.append(f'{cat} edge missing sourceId')
        if not edge.get('targetId'):
            issues.append(f'{cat} edge missing targetId')
        if not edge.get('kind'):
            issues.append(f'{cat} edge missing kind')

# packageId 一致性
pkg_ids = {p['id'] for p in d.get('packages', [])}
for i, sf in enumerate(d.get('sourceFiles', [])):
    pid = sf.get('packageId')
    if pid and pid not in pkg_ids:
        issues.append(f'sourceFile[{i}] packageId={pid} not in packages')

if issues:
    for issue in issues:
        print(f'  ISSUE: {issue}')
    sys.exit(1)
else:
    print(f'  OK: {len(d[\"sourceFiles\"])} files, {len(d[\"symbols\"])} symbols, {total_edges} edges, 0 dangling, stats consistent')
" 2>/dev/null

        if [[ $? -eq 0 ]]; then
            pass "Cangjie bridge: 结构 + 端点 + stats + kind + packageId"
        else
            fail "Cangjie bridge: 验证发现问题"
        fi
    fi

    # Cangjie 确定性（排除 generatedAt 时间戳差异）
    CJ_BRIDGE2=$($BIN_CMD_CJ analyze --root fixtures/cangjie/portable-smoke --language cangjie --format gitnexus-rc 2>/dev/null) || true
    CJ_CMP1=$(echo "$CJ_BRIDGE" | python3 -c "import json,sys; d=json.load(sys.stdin); del d['generatedAt']; print(json.dumps(d, sort_keys=True))" 2>/dev/null)
    CJ_CMP2=$(echo "$CJ_BRIDGE2" | python3 -c "import json,sys; d=json.load(sys.stdin); del d['generatedAt']; print(json.dumps(d, sort_keys=True))" 2>/dev/null)
    if [[ "$CJ_CMP1" == "$CJ_CMP2" ]]; then
        pass "Cangjie bridge: 输出确定性（排除时间戳）"
    else
        fail "Cangjie bridge: 两次运行输出不一致"
    fi
fi

echo ""

# ============================================================
# 汇总
# ============================================================
TOTAL=$((PASS + FAIL))
echo "============================================"
echo " Bridge Format 验证结果"
echo "============================================"
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo "  TOTAL: $TOTAL"
echo ""

if [[ $FAIL -gt 0 ]]; then
    echo "*** 存在失败项，Bridge 格式可能不符合下游消费预期。 ***"
    exit 1
else
    echo "全部通过 — Bridge 格式可安全用于下游消费者接入。"
    exit 0
fi
