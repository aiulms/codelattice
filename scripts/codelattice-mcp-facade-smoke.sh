#!/usr/bin/env bash
# codelattice-mcp-facade-smoke.sh
# Smoke test for 8 facade MCP tools (v0.29 consolidation)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN="$ROOT/target/debug/codelattice"

RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'
PASS=0; FAIL=0

pass() { PASS=$((PASS+1)); printf "  ${GREEN}[PASS]${NC} %s\n" "$1"; }
fail() { FAIL=$((FAIL+1)); printf "  ${RED}[FAIL]${NC} %s\n" "$1"; }

call() {
    echo "$1" | "$BIN" mcp 2>/dev/null | tail -1
}

echo "=== CodeLattice MCP Facade Smoke Test ==="

# Build if needed
if [ ! -f "$BIN" ]; then
    echo "Building codelattice..."
    cargo build --bin codelattice 2>&1 | tail -3
fi

# ── Test 1: default AI toolset is small ──────────────────────────────
echo "── Test 1: default AI toolset (small) ──"
T=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | "$BIN" mcp 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))")
[ "$T" -le 12 ] && pass "default-ai-toolset-small ($T tools)" || fail "default-ai-toolset-small (got $T)"

# ── Test 2: full toolset has 50 tools ────────────────────────────────
echo "── Test 2: full toolset (50 tools) ──"
T=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | env CODELATTICE_MCP_TOOLSET=full "$BIN" mcp 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))")
[ "$T" = "50" ] && pass "full-toolset-50" || fail "full-toolset-50 (got $T)"

# ── Test 3: core toolset sits between AI and full ────────────────────
echo "── Test 3: core toolset (middle) ──"
T=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | env CODELATTICE_MCP_TOOLSET=core "$BIN" mcp 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))")
[ "$T" -gt 12 ] && [ "$T" -lt 50 ] && pass "core-toolset-middle ($T tools)" || fail "core-toolset-middle (got $T)"

# ── Test 4: codelattice_cache explain ────────────────────────────────
echo "── Test 4: codelattice_cache explain ──"
R=$(call '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_cache","arguments":{"mode":"explain"}}}')
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion')=='facade.v1', 'no schemaVersion'
assert r.get('tool')=='codelattice_cache', 'wrong tool'
assert r.get('mode')=='explain', 'wrong mode'
assert 'underlyingTools' in r, 'no underlyingTools'
assert 'cautions' in r, 'no cautions'
print('OK')
" && pass "cache-explain" || fail "cache-explain"

# ── Test 5: codelattice_workflow onboarding ──────────────────────────
echo "── Test 5: codelattice_workflow onboarding ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_workflow\",\"arguments\":{\"mode\":\"onboarding\",\"root\":\"$ROOT/fixtures/workspace/rust-core\",\"language\":\"rust\",\"compact\":true}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion')=='ai.workflow.v1', 'wrong schema'
assert r.get('tool')=='codelattice_workflow', 'wrong tool'
assert r.get('mode')=='onboarding', 'wrong mode'
assert r.get('compact')==True, 'compact should be true'
assert isinstance(r.get('nextActions'), list) and r['nextActions'], 'missing nextActions'
assert any(a.get('tool')=='codelattice_project' for a in r['nextActions']), 'missing project nextAction'
print('OK')
" && pass "workflow-onboarding" || fail "workflow-onboarding"

# ── Test 6: invalid mode returns structured error ────────────────────
echo "── Test 6: invalid mode error ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_cache\",\"arguments\":{\"mode\":\"bogus\"}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('error')=='invalid_mode', f'expected invalid_mode, got {r.get(\"error\")}'
assert 'Valid' in r.get('message',''), 'no valid modes in message'
print('OK')
" && pass "invalid-mode-error" || fail "invalid-mode-error"

# ── Test 7: codelattice_workflow missing symbol guidance ─────────────
echo "── Test 7: workflow missing symbol guidance ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_workflow\",\"arguments\":{\"mode\":\"before_edit\",\"root\":\"$ROOT/fixtures/workspace/rust-core\",\"language\":\"rust\",\"compact\":true}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion')=='ai.workflow.v1', 'wrong schema'
assert r.get('riskLevel')=='unknown', 'missing-symbol risk should be unknown'
assert any(m.get('name')=='symbol' for m in r.get('missingInputs',[])), 'missing symbol input not reported'
assert any(a.get('tool')=='codelattice_symbol' and a.get('arguments',{}).get('mode')=='search' for a in r.get('nextActions',[])), 'missing symbol search nextAction'
print('OK')
" && pass "workflow-missing-symbol" || fail "workflow-missing-symbol"

# ── Test 8: generatedFrom fields present ─────────────────────────────
echo "── Test 8: generatedFrom fields ──"
R=$(call '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_cache","arguments":{"mode":"explain"}}}')
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
g=r.get('generatedFrom',{})
assert g.get('staticAnalysis')==True, 'staticAnalysis missing'
assert g.get('runtimeVerified')==False, 'runtimeVerified should be false'
assert g.get('scriptsExecuted')==False, 'scriptsExecuted should be false'
print('OK')
" && pass "generatedFrom-fields" || fail "generatedFrom-fields"

# ── Test 9: codelattice_project overview mode with real root ──
echo "── Test 9: codelattice_project overview ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_project\",\"arguments\":{\"root\":\"$ROOT/fixtures/workspace/rust-core\",\"mode\":\"overview\"}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('tool')=='codelattice_project', 'wrong tool'
assert r.get('mode')=='overview', 'wrong mode'
assert 'underlyingTools' in r, 'no underlyingTools'
print('OK')
" && pass "project-overview" || fail "project-overview"

# ── Test 10: codelattice_workspace graph mode ────────────────────────
echo "── Test 10: codelattice_workspace graph ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_workspace\",\"arguments\":{\"root\":\"$ROOT/fixtures/workspace\",\"mode\":\"graph\",\"compact\":true}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('tool')=='codelattice_workspace', 'wrong tool'
assert r.get('mode')=='graph', 'wrong mode'
assert 'codelattice_workspace_graph' in r.get('underlyingTools',[]), 'missing underlying'
print('OK')
" && pass "workspace-graph" || fail "workspace-graph"

# ── Test 11: codelattice_symbol search mode ──────────────────────────
echo "── Test 11: codelattice_symbol search ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_symbol\",\"arguments\":{\"root\":\"$ROOT/fixtures/workspace/rust-core\",\"mode\":\"search\",\"query\":\"main\"}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('tool')=='codelattice_symbol', 'wrong tool'
assert r.get('mode')=='search', 'wrong mode'
print('OK')
" && pass "symbol-search" || fail "symbol-search"

# ── Test 12: codelattice_workflow cross-project target guidance ──────
echo "── Test 12: workflow cross-project target guidance ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_workflow\",\"arguments\":{\"mode\":\"cross_project_impact\",\"root\":\"$ROOT/fixtures/workspace\",\"compact\":true}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion')=='ai.workflow.v1', 'wrong schema'
assert any(m.get('name')=='target' for m in r.get('missingInputs',[])), 'missing target input not reported'
assert any(a.get('tool')=='codelattice_workspace' and a.get('arguments',{}).get('mode')=='graph' for a in r.get('nextActions',[])), 'missing workspace graph nextAction'
print('OK')
" && pass "workflow-cross-project-target" || fail "workflow-cross-project-target"

# ── Test 13: codelattice_workflow execute=true runs actions ──────────
echo "── Test 13: workflow execute=true ──"
R=$(call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_workflow\",\"arguments\":{\"mode\":\"before_edit\",\"root\":\"$ROOT/fixtures/workspace/rust-core\",\"language\":\"rust\",\"symbol\":\"main\",\"execute\":true,\"compact\":true}}}")
echo "$R" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion')=='ai.workflow.v1', 'wrong schema'
assert r.get('execution',{}).get('requested') is True, 'execution not requested'
assert r.get('execution',{}).get('status')=='completed', f\"unexpected execution status {r.get('execution')}\"
assert any(a.get('tool')=='codelattice_symbol' for a in r.get('completedActions',[])), 'symbol action not completed'
assert any(a.get('tool')=='codelattice_change_review' for a in r.get('completedActions',[])), 'change review action not completed'
assert isinstance(r.get('evidence'), list) and r['evidence'], 'missing evidence'
assert 'before_edit' in r.get('answerSummary',''), 'missing answer summary'
print('OK')
" && pass "workflow-execute" || fail "workflow-execute"

# ── Results ───────────────────────────────────────────────────────────
echo ""
echo "=== Results: $PASS passed, $FAIL failed, $((PASS+FAIL)) total ==="
[ "$FAIL" -eq 0 ] && echo "ALL PASS" || { echo "SOME FAILED"; exit 1; }
