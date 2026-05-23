#!/usr/bin/env bash
# CodeLattice MCP AI Usability Smoke
# 验证 AI facade 工具的 root diagnosis / workspace misuse / semantics 等可用性硬化特性
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_BINARY="$SCRIPT_DIR/../target/release/codelattice"
if [ ! -x "$DEFAULT_BINARY" ]; then
    DEFAULT_BINARY="$SCRIPT_DIR/../target/debug/codelattice"
fi
BINARY="${CODELATTICE_MCP_BIN:-$DEFAULT_BINARY}"
SELF="$BINARY mcp --self-test 2>/dev/null"
WORKSPACE_FIXTURE="$SCRIPT_DIR/../fixtures/workspace/multi-project"

PASS=0
FAIL=0
TOTAL=0

pass() { echo "  ✅ $1"; PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); }
fail() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); }

mcp_tools_json() {
    local toolset="${1:-ai}"
    python3 - <<'PY' | CODELATTICE_MCP_TOOLSET="$toolset" "$BINARY" mcp 2>/dev/null | python3 -c '
import json, sys
for line in sys.stdin:
    if not line.strip():
        continue
    d=json.loads(line)
    if d.get("id") == 2:
        print(json.dumps({"tools": d["result"].get("tools", [])}, separators=(",", ":")))
        break
'
import json
print(json.dumps({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"ai-usability-smoke","version":"1.0"}}}))
print(json.dumps({"jsonrpc":"2.0","method":"notifications/initialized"}))
print(json.dumps({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
PY
}

mcp_call_json() {
    local tool="$1"
    local args="$2"
    local toolset="${3:-ai}"
    MCP_TOOL="$tool" MCP_ARGS="$args" python3 - <<'PY' | CODELATTICE_MCP_TOOLSET="$toolset" "$BINARY" mcp 2>/dev/null | python3 -c '
import json, sys
for line in sys.stdin:
    if not line.strip():
        continue
    d=json.loads(line)
    if d.get("id") == 2:
        if "error" in d:
            print(json.dumps({"mcpError": d["error"]}, separators=(",", ":")))
            break
        text = d["result"]["content"][0]["text"]
        print(text)
        break
'
import json, os
args = json.loads(os.environ["MCP_ARGS"])
print(json.dumps({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"ai-usability-smoke","version":"1.0"}}}))
print(json.dumps({"jsonrpc":"2.0","method":"notifications/initialized"}))
print(json.dumps({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":os.environ["MCP_TOOL"],"arguments":args}}))
PY
}

# ── 1. 工具列表验证 ──
echo "=== 1. Tool List ==="
AI_TOOLS="$(mcp_tools_json ai || echo '{"tools":[]}')"
AI_COUNT=$(echo "$AI_TOOLS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d.get('tools',[])))" 2>/dev/null || echo "0")
if [ "$AI_COUNT" = "6" ]; then
    pass "AI toolset exposes exactly 6 tools"
else
    fail "AI toolset exposes $AI_COUNT tools (expected 6)"
fi

# 6 facade descriptions 应包含 project root / workspace root 使用提示
for tool in codelattice_project codelattice_symbol codelattice_change_review codelattice_workspace codelattice_workflow; do
    DESC=$(echo "$AI_TOOLS" | python3 -c "
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
SINGLE_OUTPUT=$(mcp_call_json codelattice_project "$(printf '{"root":"%s/fixtures/javascript/portable-smoke","compact":true}' "$SCRIPT_DIR/..")" ai || echo "{}")
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
    WS_OUTPUT=$(mcp_call_json codelattice_workspace "$(printf '{"root":"%s","mode":"overview","compact":true}' "$WORKSPACE_FIXTURE")" ai || echo "{}")
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
# 检查所有 6 个 AI facade 输出都包含 analysisSemantics；每个工具使用合法 mode。
check_semantics() {
    local tool="$1"
    local args="$2"
    SEM=$(mcp_call_json "$tool" "$args" ai | python3 -c "
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
}
PROJECT_ROOT="$SCRIPT_DIR/../fixtures/javascript/portable-smoke"
check_semantics codelattice_project "$(printf '{"root":"%s","compact":true,"mode":"overview"}' "$PROJECT_ROOT")"
check_semantics codelattice_symbol "$(printf '{"root":"%s","compact":true,"mode":"search","query":"logger"}' "$PROJECT_ROOT")"
check_semantics codelattice_change_review "$(printf '{"root":"%s","compact":true,"mode":"impact","symbol":"logger"}' "$PROJECT_ROOT")"
check_semantics codelattice_workspace "$(printf '{"root":"%s","compact":true,"mode":"overview"}' "$WORKSPACE_FIXTURE")"
check_semantics codelattice_cache '{"mode":"status","compact":true}'
check_semantics codelattice_workflow "$(printf '{"root":"%s","compact":true,"mode":"onboarding"}' "$PROJECT_ROOT")"

if [ -d "$WORKSPACE_FIXTURE" ]; then
    AUTO_OUT=$(mcp_call_json codelattice_project "$(printf '{"root":"%s","compact":true}' "$WORKSPACE_FIXTURE")" ai || echo "{}")
    if echo "$AUTO_OUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('schemaVersion') == 'codelattice.workspaceAutoEntry.v1'
assert d.get('rootDiagnosis',{}).get('kind') in ('workspace','unsupported_or_mixed_workspace')
assert d.get('analysisSemantics',{}).get('staticAnalysisExecuted') is True
print('OK')
" 2>/dev/null; then
        pass "workspace auto-entry includes rootDiagnosis and analysisSemantics"
    else
        fail "workspace auto-entry missing rootDiagnosis/analysisSemantics"
    fi
fi

if [ -d "$WORKSPACE_FIXTURE" ]; then
    WF_WS=$(mcp_call_json codelattice_workflow "$(printf '{"root":"%s","compact":true,"mode":"before_edit","symbol":"PolicyDecision"}' "$WORKSPACE_FIXTURE")" ai || echo "{}")
    if echo "$WF_WS" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('rootDiagnosis',{}).get('kind') in ('workspace','unsupported_or_mixed_workspace')
assert d.get('analysisSemantics',{}).get('staticAnalysisExecuted') is True
root = d.get('root')
for action in d.get('nextActions', []):
    if action.get('tool') in ('codelattice_symbol', 'codelattice_change_review', 'codelattice_project'):
        assert action.get('arguments',{}).get('root') != root, action
print('OK')
" 2>/dev/null; then
        pass "workflow on workspace root avoids direct project/symbol actions with workspace root"
    else
        fail "workflow on workspace root still routes project/symbol tools to workspace root"
    fi
fi

# ── 4. generatedFrom 兼容性 ──
echo "=== 4. generatedFrom compatibility ==="
GF=$(mcp_call_json codelattice_project "$(printf '{"root":"%s/fixtures/javascript/portable-smoke","compact":true}' "$SCRIPT_DIR/..")" ai | python3 -c "
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
CS=$(mcp_call_json codelattice_cache '{"mode":"status","compact":true}' ai | python3 -c "
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
HINT=$(mcp_call_json codelattice_cache '{"mode":"status","compact":true}' ai | python3 -c "
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
FULL_COUNT=$(mcp_tools_json full | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('tools',[])))" 2>/dev/null || echo "0")
if [ "$FULL_COUNT" -gt 40 ]; then
    pass "full toolset exposes $FULL_COUNT tools (≥41)"
else
    fail "full toolset only exposes $FULL_COUNT tools"
fi

# ── 7. Workspace symbol misuse ──
echo "=== 7. Workspace symbol misuse ==="
if [ -d "$WORKSPACE_FIXTURE" ]; then
    SYM_OUT=$(mcp_call_json codelattice_symbol "$(printf '{"root":"%s","mode":"search","query":"nonexistent_symbol_xyz123","compact":true}' "$WORKSPACE_FIXTURE")" ai || echo "{}")
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
