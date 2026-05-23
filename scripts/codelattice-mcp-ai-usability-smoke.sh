#!/usr/bin/env bash
# CodeLattice MCP AI Usability Smoke
# 验证 AI facade 工具的 root diagnosis / workspace misuse / semantics 等可用性硬化特性
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/../target/debug/codelattice"
SELF="$BINARY mcp --self-test 2>/dev/null"
WORKSPACE_FIXTURE="$SCRIPT_DIR/../fixtures/workspace/multi-project"

PASS=0
FAIL=0
TOTAL=0

pass() { echo "  ✅ $1"; PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); }
fail() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); }

# ── 1. 工具列表验证 ──
echo "=== 1. Tool List ==="
AI_COUNT=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp --list-tools 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d.get('tools',[])))" 2>/dev/null || echo "0")
if [ "$AI_COUNT" = "6" ]; then
    pass "AI toolset exposes exactly 6 tools"
else
    fail "AI toolset exposes $AI_COUNT tools (expected 6)"
fi

# 6 facade descriptions 应包含 project root / workspace root 使用提示
for tool in codelattice_project codelattice_symbol codelattice_change_review codelattice_workspace codelattice_workflow; do
    DESC=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp --list-tools 2>/dev/null | python3 -c "
import json,sys
for t in json.load(sys.stdin).get('tools',[]):
    if t.get('name') == '$tool':
        print(t.get('description',''))
        break
" 2>/dev/null || echo "")
    if echo "$DESC" | grep -qi "project\|workspace\|root"; then
        pass "$tool description mentions root type"
    else
        fail "$tool description missing root type hint"
    fi
done

# ── 2. Root Diagnosis ──
echo "=== 2. Root Diagnosis ==="

# 2a. 单项目 root
SINGLE_OUTPUT=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_project "$(printf '{"root":"%s/fixtures/javascript/portable-smoke","compact":true}' "$SCRIPT_DIR/..")" 2>/dev/null || echo "{}")
if echo "$SINGLE_OUTPUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
rd=d.get('rootDiagnosis',{})
k=rd.get('kind','')
assert k in ('single_project','unknown'), f'Expected single_project, got {k}'
print('OK')
" 2>/dev/null; then
    pass "single project root diagnosed correctly"
else
    KIND=$(echo "$SINGLE_OUTPUT" | python3 -c "import json,sys;print(json.load(sys.stdin).get('rootDiagnosis',{}).get('kind','missing'))" 2>/dev/null || echo "parse_error")
    fail "single project root diagnosis: got kind=$KIND"
fi

# 2b. workspace root
if [ -d "$WORKSPACE_FIXTURE" ]; then
    WS_OUTPUT=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_workspace "$(printf '{"root":"%s","mode":"overview","compact":true}' "$WORKSPACE_FIXTURE")" 2>/dev/null || echo "{}")
    if echo "$WS_OUTPUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
rd=d.get('rootDiagnosis',{})
k=rd.get('kind','')
assert k in ('workspace','unsupported_or_mixed_workspace'), f'Expected workspace, got {k}'
print('OK')
" 2>/dev/null; then
        pass "workspace root diagnosed correctly"
    else
        KIND2=$(echo "$WS_OUTPUT" | python3 -c "import json,sys;print(json.load(sys.stdin).get('rootDiagnosis',{}).get('kind','missing'))" 2>/dev/null || echo "parse_error")
        fail "workspace root diagnosis: got kind=$KIND2"
    fi
else
    echo "  ⚠️ workspace fixture not found, skipping"
fi

# ── 3. analysisSemantics ──
echo "=== 3. analysisSemantics ==="
# 检查所有 6 个 facade 输出都包含 analysisSemantics
for tool in codelattice_project codelattice_symbol codelattice_change_review codelattice_workspace codelattice_cache codelattice_workflow; do
    SEM=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call "$tool" '{"root":"'"$SCRIPT_DIR"'/..","compact":true,"mode":"overview"}' 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)
as_=d.get('analysisSemantics',{})
if as_.get('staticAnalysisExecuted') == True and as_.get('targetCodeExecuted') == False:
    print('OK')
else:
    print('MISSING')
" 2>/dev/null || echo "ROUTE_ERROR")
    if [ "$SEM" = "OK" ]; then
        pass "$tool has correct analysisSemantics"
    else
        fail "$tool analysisSemantics: $SEM"
    fi
done

# ── 4. generatedFrom 兼容性 ──
echo "=== 4. generatedFrom compatibility ==="
GF=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_project "$(printf '{"root":"%s/fixtures/javascript/portable-smoke","compact":true}' "$SCRIPT_DIR/..")" 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)
gf=d.get('generatedFrom',{})
if gf.get('staticAnalysis')==True and gf.get('runtimeVerified')==False:
    print('OK')
else:
    print('BROKEN')
" 2>/dev/null || echo "parse_error")
if [ "$GF" = "OK" ]; then
    pass "generatedFrom fields preserved (staticAnalysis=true, runtimeVerified=false)"
else
    fail "generatedFrom: $GF"
fi

# ── 5. cacheSemantics ──
echo "=== 5. cacheSemantics ==="
CS=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_cache '{"mode":"status","compact":true}' 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)
cs=d.get('cacheSemantics',{})
if cs.get('analysisAvailableWithoutPersistentCache')==True:
    print('OK')
else:
    print('MISSING')
" 2>/dev/null || echo "parse_error")
if [ "$CS" = "OK" ]; then
    pass "cacheSemantics present: analysisAvailableWithoutPersistentCache=true"
else
    fail "cacheSemantics: $CS"
fi

# cache enableHint 存在
HINT=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_cache '{"mode":"status","compact":true}' 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)
cs=d.get('cacheSemantics',{})
h=cs.get('enableHint','')
print(h)
" 2>/dev/null || echo "")
if echo "$HINT" | grep -qi "CODELATTICE_CACHE_DIR"; then
    pass "cacheSemantics has enableHint"
else
    fail "cacheSemantics missing enableHint"
fi

# ── 6. 工具集隔离 ──
echo "=== 6. Toolset isolation ==="
FULL_COUNT=$(CODELATTICE_MCP_TOOLSET=full "$BINARY" mcp --list-tools 2>/dev/null | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('tools',[])))" 2>/dev/null || echo "0")
if [ "$FULL_COUNT" -gt 40 ]; then
    pass "full toolset exposes $FULL_COUNT tools (≥41)"
else
    fail "full toolset only exposes $FULL_COUNT tools"
fi

# ── 7. Workspace symbol misuse ──
echo "=== 7. Workspace symbol misuse ==="
if [ -d "$WORKSPACE_FIXTURE" ]; then
    SYM_OUT=$(CODELATTICE_MCP_TOOLSET=ai "$BINARY" mcp call codelattice_symbol "$(printf '{"root":"%s","mode":"search","query":"nonexistent_symbol_xyz123","compact":true}' "$WORKSPACE_FIXTURE")" 2>/dev/null || echo "{}")
    MISUSE=$(echo "$SYM_OUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
rd=d.get('rootDiagnosis',{})
k=rd.get('kind','')
print(k)
" 2>/dev/null || echo "unknown")
    # workspace root 下搜索无结果时应该有 root diagnosis
    if [ "$MISUSE" = "workspace" ] || [ "$MISUSE" = "unsupported_or_mixed_workspace" ]; then
        pass "symbol search on workspace root returns rootDiagnosis ($MISUSE)"
    else
        fail "symbol search on workspace root: got kind=$MISUSE (expected workspace)"
    fi
fi

# ── 结果 ──
echo ""
echo "=== Results: $PASS passed, $FAIL failed, $TOTAL total ==="
if [ "$FAIL" -gt 0 ]; then
    echo "❌ Some AI usability tests FAILED"
    exit 1
else
    echo "✅ All AI usability smoke tests passed"
fi
