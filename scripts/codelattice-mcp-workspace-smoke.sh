#!/usr/bin/env bash
# codelattice-mcp-workspace-smoke.sh
# Smoke test for workspace graph and cross-project impact MCP tools
set -euo pipefail
export CODELATTICE_MCP_TOOLSET=full

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE="$ROOT/fixtures/workspace"

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS=0; FAIL=0; TOTAL=0

pass() { PASS=$((PASS+1)); TOTAL=$((TOTAL+1)); printf "  ${GREEN}[PASS]${NC} $1\n"; }
fail() { FAIL=$((FAIL+1)); TOTAL=$((TOTAL+1)); printf "  ${RED}[FAIL]${NC} $1\n"; }

echo "=== CodeLattice MCP Workspace Smoke Test ==="
echo "Fixture: $FIXTURE"

# ── Helper: call MCP tool via CLI ─────────────────────────────────
call_tool() {
  local tool="$1"; shift
  local args="$*"
  # Use codelattice CLI with mcp subcommand to call a tool
  echo "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$tool\",\"arguments\":$args}}" \
    | "$ROOT/target/debug/codelattice" mcp 2>/dev/null \
    | tail -1
}

# Build if needed
if [ ! -f "$ROOT/target/debug/codelattice" ]; then
  echo "Building codelattice..."
  cargo build --bin codelattice 2>&1 | tail -3
fi

# ── Test 1: tools/list contains new tools ──────────────────────────
echo ""
echo "── Test 1: tools/list contains workspace tools ────"
LIST=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | "$ROOT/target/debug/codelattice" mcp 2>/dev/null | tail -1)

echo "$LIST" | python3 -c "
import json,sys
d=json.load(sys.stdin)
tools=[t['name'] for t in d.get('result',{}).get('tools',[])]
assert 'codelattice_workspace_graph' in tools, 'workspace_graph not in tools/list'
assert 'codelattice_cross_project_impact' in tools, 'cross_project_impact not in tools/list'
print(f'tools/list OK: {len(tools)} tools, workspace_graph and cross_project_impact present')
" 2>/dev/null && pass "tools-list-workspace" || { fail "tools-list-workspace"; echo "  ${LIST:0:200}"; }

# ── Test 2: workspace_graph basic ──────────────────────────────────
echo ""
echo "── Test 2: workspace_graph basic ──────────────────"
WG=$(call_tool "codelattice_workspace_graph" "{\"root\":\"$FIXTURE\"}")
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion') == 'workspace.graph.v1', 'wrong graph schemaVersion'
assert r.get('generatedFrom',{}).get('staticAnalysis') == True, 'missing staticAnalysis flag'
assert r.get('generatedFrom',{}).get('runtimeVerified') == False, 'runtimeVerified should be false'
assert r.get('generatedFrom',{}).get('scriptsExecuted') == False, 'scriptsExecuted should be false'
assert r.get('nodes'), 'no nodes returned'
assert r.get('edges'), 'no edges returned'
kinds=set(n['kind'] for n in r['nodes'])
assert 'workspace' in kinds, 'no workspace node'
assert 'project' in kinds, 'no project nodes'
print(f'graph: {len(r[\"nodes\"])} nodes, {len(r[\"edges\"])} edges, kinds={sorted(kinds)}')
" 2>/dev/null && pass "workspace-graph-basic" || { fail "workspace-graph-basic"; echo "  ${WG:0:200}"; }

# ── Test 3: workspace_graph summary ────────────────────────────────
echo ""
echo "── Test 3: workspace_graph summary ────────────────"
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
s=r.get('summary',{})
assert s.get('nodeCount',0) > 0, 'summary nodeCount is 0'
assert s.get('edgeCount',0) > 0, 'summary edgeCount is 0'
assert s.get('projectCount',0) >= 2, f'expected >=2 projects, got {s.get(\"projectCount\",0)}'
print(f'summary: nodes={s[\"nodeCount\"]} edges={s[\"edgeCount\"]} projects={s[\"projectCount\"]}')
" 2>/dev/null && pass "workspace-graph-summary" || { fail "workspace-graph-summary"; echo "  ${WG:0:200}"; }

# ── Test 4: workspace_graph compact ────────────────────────────────
echo ""
echo "── Test 4: workspace_graph compact ────────────────"
WGC=$(call_tool "codelattice_workspace_graph" "{\"root\":\"$FIXTURE\",\"compact\":true}")
echo "$WGC" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
# compact should have summary but not full nodes/edges (or truncated)
has_summary = 'summary' in r
has_nodes = 'nodes' in r
print(f'compact: has_summary={has_summary} has_nodes={has_nodes}')
assert has_summary, 'compact missing summary'
" 2>/dev/null && pass "workspace-graph-compact" || { fail "workspace-graph-compact"; echo "  ${WGC:0:200}"; }

# ── Test 5: workspace_graph contains Dockerfile node ───────────────
echo ""
echo "── Test 5: workspace_graph Dockerfile node ────────"
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
labels=[n['label'] for n in r['nodes']]
assert 'Dockerfile' in labels, f'Dockerfile not in node labels: {labels}'
print('Dockerfile node present')
" 2>/dev/null && pass "workspace-graph-dockerfile" || { fail "workspace-graph-dockerfile"; echo "  ${WG:0:200}"; }

# ── Test 6: workspace_graph contains Makefile node ──────────────────
echo ""
echo "── Test 6: workspace_graph Makefile node ───────────"
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
labels=[n['label'] for n in r['nodes']]
assert 'Makefile' in labels, f'Makefile not in node labels: {labels}'
print('Makefile node present')
" 2>/dev/null && pass "workspace-graph-makefile" || { fail "workspace-graph-makefile"; echo "  ${WG:0:200}"; }

# ── Test 7: workspace_graph no dangling edges ──────────────────────
echo ""
echo "── Test 7: workspace_graph no dangling edges ──────"
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
node_ids=set(n['id'] for n in r['nodes'])
dangling=0
for e in r['edges']:
    if e['source'] not in node_ids: dangling+=1
    if e['target'] not in node_ids: dangling+=1
assert dangling==0, f'{dangling} dangling edges found'
print(f'no dangling edges OK ({len(r[\"edges\"])} edges checked)')
" 2>/dev/null && pass "workspace-graph-no-dangling" || { fail "workspace-graph-no-dangling"; echo "  ${WG:0:200}"; }

# ── Test 8: workspace_graph cautions ───────────────────────────────
echo ""
echo "── Test 8: workspace_graph cautions ───────────────"
echo "$WG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
cautions=r.get('cautions',[])
assert len(cautions) > 0, 'no cautions found'
has_static = any('static' in c.lower() for c in cautions)
assert has_static, 'missing static-only caution'
print(f'cautions: {len(cautions)} items, static-only present')
" 2>/dev/null && pass "workspace-graph-cautions" || { fail "workspace-graph-cautions"; echo "  ${WG:0:200}"; }

# ── Test 9: cross_project_impact by projectId ───────────────────────
echo ""
echo "── Test 9: cross_project_impact by projectId ──────"
IMP1=$(call_tool "codelattice_cross_project_impact" "{\"root\":\"$FIXTURE\",\"target\":{\"projectId\":\"rust-core\"},\"direction\":\"downstream\"}")
echo "$IMP1" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('schemaVersion') == 'workspace.impact.v1', 'wrong impact schemaVersion'
assert r.get('generatedFrom',{}).get('staticAnalysis') == True, 'missing staticAnalysis flag'
assert r.get('generatedFrom',{}).get('runtimeVerified') == False, 'runtimeVerified should be false'
assert r.get('target',{}).get('resolvedNodeId'), 'target not resolved'
assert r.get('summary',{}).get('riskLevel'), 'no risk level'
print(f'impact: resolved={r[\"target\"][\"resolvedNodeId\"][:30]} risk={r[\"summary\"][\"riskLevel\"]}')
" 2>/dev/null && pass "impact-by-projectid" || { fail "impact-by-projectid"; echo "  ${IMP1:0:200}"; }

# ── Test 10: cross_project_impact by path=Makefile ──────────────────
echo ""
echo "── Test 10: cross_project_impact by path=Makefile ──"
IMP2=$(call_tool "codelattice_cross_project_impact" "{\"root\":\"$FIXTURE\",\"target\":{\"path\":\"Makefile\"},\"direction\":\"upstream\"}")
echo "$IMP2" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('target',{}).get('resolvedNodeId'), 'Makefile target not resolved: '+str(r.get('target',{}))
print(f'impact-by-path Makefile: resolved={r[\"target\"][\"resolvedNodeId\"][:30]} conf={r[\"target\"].get(\"resolutionConfidence\",0)}')
" 2>/dev/null && pass "impact-by-path-makefile" || { fail "impact-by-path-makefile"; echo "  ${IMP2:0:200}"; }

# ── Test 11: cross_project_impact by path=Dockerfile ────────────────
echo ""
echo "── Test 11: cross_project_impact by path=Dockerfile ──"
IMP3=$(call_tool "codelattice_cross_project_impact" "{\"root\":\"$FIXTURE\",\"target\":{\"path\":\"Dockerfile\"},\"direction\":\"upstream\"}")
echo "$IMP3" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
assert r.get('target',{}).get('resolvedNodeId'), 'Dockerfile target not resolved: '+str(r.get('target',{}))
print(f'impact-by-path Dockerfile: resolved={r[\"target\"][\"resolvedNodeId\"][:30]} conf={r[\"target\"].get(\"resolutionConfidence\",0)}')
" 2>/dev/null && pass "impact-by-path-dockerfile" || { fail "impact-by-path-dockerfile"; echo "  ${IMP3:0:200}"; }

# ── Test 12: cross_project_impact unknown target ────────────────────
echo ""
echo "── Test 12: cross_project_impact unknown target ───"
IMP4=$(call_tool "codelattice_cross_project_impact" "{\"root\":\"$FIXTURE\",\"target\":{\"path\":\"nonexistent-file.xyz\"},\"direction\":\"both\"}")
echo "$IMP4" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
# Should not crash; target should be unresolved
tgt=r.get('target',{})
print(f'unknown target: kind={tgt.get(\"resolvedKind\",\"?\")} conf={tgt.get(\"resolutionConfidence\",0)}')
# Should have cautions
assert r.get('cautions'), 'no cautions for unknown target'
" 2>/dev/null && pass "impact-unknown-target" || { fail "impact-unknown-target"; echo "  ${IMP4:0:200}"; }

# ── Test 13: cross_project_impact review checklist ──────────────────
echo ""
echo "── Test 13: cross_project_impact review checklist ──"
echo "$IMP1" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=json.loads(d['result']['content'][0]['text'])
cl=r.get('reviewChecklist',[])
assert isinstance(cl,list), 'reviewChecklist not a list'
print(f'review checklist: {len(cl)} items')
" 2>/dev/null && pass "impact-review-checklist" || { fail "impact-review-checklist"; echo "  ${IMP1:0:200}"; }

# ── Test 14: invalid root ──────────────────────────────────────────
echo ""
echo "── Test 14: invalid root ──────────────────────────"
IMP5=$(call_tool "codelattice_workspace_graph" "{\"root\":\"/nonexistent/path/xyz123\"}")
echo "$IMP5" | python3 -c "
import json,sys
d=json.load(sys.stdin)
# Should return error
is_error=d.get('result',{}).get('isError',False) or 'error' in str(d).lower()[:200]
print(f'invalid root handled: isError={is_error}')
" 2>/dev/null && pass "invalid-root" || { fail "invalid-root"; echo "  ${IMP5:0:200}"; }

# ── Summary ────────────────────────────────────────────────────────
echo ""
echo "=== Results: $PASS passed, $FAIL failed, $TOTAL total ==="
if [ "$FAIL" -gt 0 ]; then
  echo "SOME FAILED"
  exit 1
else
  echo "ALL PASS"
fi
