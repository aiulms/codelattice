#!/usr/bin/env bash
set -euo pipefail
# CodeLattice WebUI Workspace Cross-Project Impact Smoke Test
# Tests workspace impact API: target resolution, BFS traversal, risk scoring, error handling.
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 28765)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
TD=$(mktemp -d /tmp/cls-wsi.XXXXXX)
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD" "$WF"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "CodeLattice Workspace Impact Smoke"

# ── Fixture: mixed workspace ──────────────────────────────────────
WF=$(mktemp -d /tmp/cls-wsi-fixture.XXXXXX)

# Rust project
mkdir -p "$WF/rust-core/src"
cat > "$WF/rust-core/Cargo.toml" <<'EOF'
[package]
name = "rust-core"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF
echo 'fn main() { println!("hello"); }' > "$WF/rust-core/src/main.rs"

# TypeScript project with workspace ref
mkdir -p "$WF/ts-ui/src"
cat > "$WF/ts-ui/package.json" <<'EOF'
{
  "name": "ts-ui",
  "version": "1.0.0",
  "scripts": {
    "build": "./scripts/build-core.sh",
    "deploy": "bash scripts/deploy.sh"
  }
}
EOF
echo '{}' > "$WF/ts-ui/tsconfig.json"
echo 'export {}' > "$WF/ts-ui/src/index.ts"

# Shell scripts
mkdir -p "$WF/scripts"
echo '#!/bin/bash' > "$WF/scripts/build-core.sh"
echo 'cd ../rust-core && cargo build' >> "$WF/scripts/build-core.sh"
echo '#!/bin/bash' > "$WF/scripts/deploy.sh"
echo 'source ./scripts/build-core.sh' > "$WF/scripts/deploy.sh"

# CI
mkdir -p "$WF/.github/workflows"
cat > "$WF/.github/workflows/ci.yml" <<'EOF'
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: ./scripts/build-core.sh
EOF

# Dockerfile
cat > "$WF/Dockerfile" <<'EOF'
FROM rust:1.70
COPY rust-core /app
WORKDIR /app
RUN cargo build --release
EOF

# Unsupported
mkdir -p "$WF/unsupported-csharp"
cat > "$WF/unsupported-csharp/demo.csproj" <<'EOF'
<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType></PropertyGroup></Project>
EOF

# ── Start Runner ──────────────────────────────────────────────────
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null || { fail "not started"; exit 1; }
pass "started"
BASE="http://127.0.0.1:$PORT"

# ── Health ────────────────────────────────────────────────────────
curl -s "$BASE/api/health"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "health"||fail "health"

# ── Inventory ─────────────────────────────────────────────────────
INV=$(curl -s "$BASE/api/workspace/inventory?root=$WF")
echo "$INV" | python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "inventory"||fail "inventory"

# ── Analyze ───────────────────────────────────────────────────────
AN=$(curl -s -X POST "$BASE/api/workspace/analyze" -H "Content-Type: application/json" -d "{\"root\":\"$WF\",\"mode\":\"recommended\",\"redactRoot\":true}")
WID=$(echo "$AN"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['workspaceId'])" 2>/dev/null)
[[ -n "$WID" ]] && pass "analyze" || { fail "analyze"; echo "  ${AN:0:200}"; }

# ── Graph (pre-req) ──────────────────────────────────────────────
GR=$(curl -s "$BASE/api/workspace/graph?runId=$WID")
echo "$GR" | python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];g=d['data'];assert g['summary']['nodeCount']>0" 2>/dev/null && pass "graph-ok"||fail "graph-ok"

if [[ -z "$WID" ]]; then
  echo "FATAL: no workspace ID, cannot continue"
  exit 1
fi

# ── Test 1: Impact by projectId (POST) ────────────────────────────
IMP1=$(curl -s -X POST "$BASE/api/workspace/impact" -H "Content-Type: application/json" \
  -d "{\"workspaceRunId\":\"$WID\",\"target\":{\"projectId\":\"rust-core\"},\"direction\":\"both\",\"maxDepth\":3}")
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'impact failed: ' + d.get('error','')
imp=d['data']
assert imp['schemaVersion']=='workspace.impact.v1', 'bad schema: '+imp.get('schemaVersion','')
tgt=imp['target']
assert tgt['resolvedNodeId'] is not None, 'target not resolved: '+str(tgt)
assert tgt['resolutionConfidence']>0, 'bad conf: '+str(tgt.get('resolutionConfidence'))
print('target: '+tgt.get('label','?')+' kind='+tgt.get('resolvedKind','?')+' conf='+str(tgt.get('resolutionConfidence')))
" 2>/dev/null && pass "impact-by-projectid" || { fail "impact-by-projectid"; echo "  ${IMP1:0:300}"; }

# ── Test 2: Impact summary fields ─────────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data'];sm=d['summary']
assert 'riskLevel' in sm, 'missing riskLevel'
assert sm['riskLevel'] in ('low','medium','high','critical','unknown'), 'bad risk: '+sm['riskLevel']
assert 'confidence' in sm, 'missing confidence'
assert 'affectedProjectCount' in sm, 'missing affectedProjectCount'
assert 'edgeCountConsidered' in sm, 'missing edgeCountConsidered'
print('risk='+sm['riskLevel']+' conf='+sm['confidence']+' projs='+str(sm['affectedProjectCount']))
" 2>/dev/null && pass "impact-summary" || fail "impact-summary"

# ── Test 3: affectedProjects exists ───────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
ap=d.get('affectedProjects',[])
ac=d.get('affectedConfigs',[])
as_=d.get('affectedScripts',[])
aw=d.get('affectedWorkflows',[])
ub=d.get('unsupportedBoundaries',[])
# At minimum, there should be some affected entries or paths
total = len(ap)+len(ac)+len(as_)+len(aw)+len(ub)
print('projects=%d configs=%d scripts=%d workflows=%d boundaries=%d total=%d' % (len(ap),len(ac),len(as_),len(aw),len(ub),total))
" 2>/dev/null && pass "impact-affected-lists" || fail "impact-affected-lists"

# ── Test 4: riskReasons array ─────────────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
rr=d.get('riskReasons',[])
assert isinstance(rr,list), 'riskReasons not a list'
print('riskReasons: '+str(len(rr))+' items')
" 2>/dev/null && pass "risk-reasons" || fail "risk-reasons"

# ── Test 5: reviewChecklist array ─────────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
rc=d.get('reviewChecklist',[])
assert isinstance(rc,list) and len(rc)>0, 'reviewChecklist empty or missing'
print('checklist: '+str(len(rc))+' items')
" 2>/dev/null && pass "review-checklist" || fail "review-checklist"

# ── Test 6: generatedFrom flags ───────────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
gf=d['generatedFrom']
assert gf['scriptsExecuted']==False, 'scriptsExecuted should be False'
assert gf['runtimeVerified']==False, 'runtimeVerified should be False'
assert gf['staticAnalysis']==True, 'staticAnalysis should be True'
print('generatedFrom OK')
" 2>/dev/null && pass "generated-from" || fail "generated-from"

# ── Test 7: Impact by GET ─────────────────────────────────────────
IMP2=$(curl -s "$BASE/api/workspace/impact?runId=$WID&projectId=ts-ui&direction=downstream")
echo "$IMP2" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'GET impact failed: '+d.get('error','')
imp=d['data']
assert imp['summary']['direction']=='downstream', 'direction mismatch: '+imp['summary']['direction']
print('GET impact: risk='+imp['summary']['riskLevel'])
" 2>/dev/null && pass "impact-get" || { fail "impact-get"; echo "  ${IMP2:0:200}"; }

# ── Test 8: Impact unknown target ─────────────────────────────────
IMP3=$(curl -s -X POST "$BASE/api/workspace/impact" -H "Content-Type: application/json" \
  -d "{\"workspaceRunId\":\"$WID\",\"target\":{\"projectId\":\"nonexistent-xyz-123\"},\"direction\":\"both\"}")
echo "$IMP3" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'should return success with unknown target, not error'
imp=d['data']
tgt=imp['target']
assert tgt.get('resolvedKind') in ('unknown',None,''), 'should be unknown: '+str(tgt.get('resolvedKind'))
assert imp['summary']['riskLevel']=='unknown', 'should be unknown risk'
print('unknown target handled OK')
" 2>/dev/null && pass "impact-unknown-target" || { fail "impact-unknown-target"; echo "  ${IMP3:0:200}"; }

# ── Test 9: Invalid run ID ────────────────────────────────────────
IMP4=$(curl -s "$BASE/api/workspace/impact?runId=nonexistent0000&projectId=rust-core")
echo "$IMP4" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert not d['success'], 'should fail for invalid run'
assert d.get('status')==404, 'should be 404: '+str(d.get('status'))
print('invalid run 404 OK')
" 2>/dev/null && pass "impact-invalid-run" || fail "impact-invalid-run"

# ── Test 10: Missing target ───────────────────────────────────────
IMP5=$(curl -s -X POST "$BASE/api/workspace/impact" -H "Content-Type: application/json" \
  -d "{\"workspaceRunId\":\"$WID\",\"target\":{},\"direction\":\"both\"}")
echo "$IMP5" | python3 -c "
import json,sys;d=json.load(sys.stdin)
# Empty target should either resolve as unknown or return error
# Either way, should not crash
print('empty target handled')
" 2>/dev/null && pass "impact-empty-target" || fail "impact-empty-target"

# ── Test 11: Invalid direction ────────────────────────────────────
IMP6=$(curl -s "$BASE/api/workspace/impact?runId=$WID&projectId=rust-core&direction=sideways")
echo "$IMP6" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert not d['success'], 'should fail for invalid direction'
print('invalid direction rejected')
" 2>/dev/null && pass "impact-invalid-direction" || fail "impact-invalid-direction"

# ── Test 12: Insights with impact hints ───────────────────────────
INS=$(curl -s "$BASE/api/workspace/insights?runId=$WID")
echo "$INS" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'insights failed'
hints=d['data'].get('crossProjectImpactHints',{})
assert isinstance(hints,dict), 'impact hints missing or not dict'
# Either available=true with data, or available=false with reason
print('impact hints: available='+str(hints.get('available')))
" 2>/dev/null && pass "insights-impact-hints" || { fail "insights-impact-hints"; echo "  ${INS:0:200}"; }

# ── Test 13: No dangling paths ────────────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
for p in d.get('paths',[]):
    assert p.get('from'), 'path missing from'
    assert p.get('to'), 'path missing to'
    assert p.get('edges'), 'path missing edges'
    for e in p['edges']:
        assert e.get('kind'), 'edge missing kind'
        assert e.get('source'), 'edge missing source'
        assert e.get('target'), 'edge missing target'
print('paths OK, no dangling')
" 2>/dev/null && pass "no-dangling-paths" || fail "no-dangling-paths"

# ── Test 14: Caution messages present ─────────────────────────────
echo "$IMP1" | python3 -c "
import json,sys;d=json.load(sys.stdin)['data']
cauts=d.get('cautions',[])
assert isinstance(cauts,list) and len(cauts)>0, 'missing cautions'
has_static = any('static' in c.lower() or 'heuristic' in c.lower() for c in cauts)
assert has_static, 'no static-only caution found'
print('cautions: '+str(len(cauts))+' items, static-only present')
" 2>/dev/null && pass "cautions-present" || fail "cautions-present"

# ── Test 15: Impact by path (Dockerfile IS in graph as config node) ─
  IMP7=$(curl -s -X POST "$BASE/api/workspace/impact" -H "Content-Type: application/json" \
    -d "{\"workspaceRunId\":\"$WID\",\"target\":{\"path\":\"Dockerfile\"},\"direction\":\"upstream\"}")
  echo "$IMP7" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'impact by path failed: ' + d.get('error','')
tgt=d['data']['target']
assert tgt.get('resolvedNodeId'), 'path target not resolved: '+str(tgt)
print('path target: '+tgt.get('label','?')+' conf='+str(tgt.get('resolutionConfidence',0)))
" 2>/dev/null && pass "impact-by-path" || { fail "impact-by-path"; echo "  ${IMP7:0:200}"; }

# ── Test 16: Direction upstream ───────────────────────────────────
IMP8=$(curl -s -X POST "$BASE/api/workspace/impact" -H "Content-Type: application/json" \
  -d "{\"workspaceRunId\":\"$WID\",\"target\":{\"projectId\":\"ts-ui\"},\"direction\":\"upstream\"}")
echo "$IMP8" | python3 -c "
import json,sys;d=json.load(sys.stdin)
assert d['success'], 'upstream impact failed'
sm=d['data']['summary']
assert sm['direction']=='upstream', 'direction should be upstream: '+sm['direction']
print('upstream: risk='+sm['riskLevel']+' projs='+str(sm['affectedProjectCount']))
" 2>/dev/null && pass "impact-upstream" || { fail "impact-upstream"; echo "  ${IMP8:0:200}"; }

# ── Summary ───────────────────────────────────────────────────────
echo ""
echo "Results: $P passed, $F failed"
if [[ $F -eq 0 ]]; then
  echo "ALL PASS"
  exit 0
else
  echo "SOME FAILED"
  exit 1
fi
