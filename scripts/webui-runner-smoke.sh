#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 28765)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
TD=$(mktemp -d /tmp/cls-run.XXXXXX)
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "CodeLattice Runner Smoke (Phase F)"
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null || { fail "not started"; exit 1; }
pass "started"
BASE="http://127.0.0.1:$PORT"

# Health (Phase E: {success,data})
curl -s "$BASE/api/health"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and d['data']['status']=='ok'" 2>/dev/null && pass "health"||fail "health"

# Generate snapshot (Phase E: data.id)
FIXTURE="$WS/fixtures/rust/portable-smoke"
GEN=$(curl -s -X POST "$BASE/api/generate-snapshot" -H "Content-Type: application/json" -d "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"full\":true,\"redactRoot\":true}")
SID=$(echo "$GEN"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
[[ -n "$SID" ]] && pass "generate: $SID" || { fail "generate"; echo "  ${GEN:0:200}"; }

# List snapshots (Phase E: data[])
SL=$(curl -s "$BASE/api/snapshots")
SC=$(echo "$SL"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$SC" -ge 1 ]] && pass "list: $SC" || fail "list: $SC"

# Snapshot detail (Phase E: data)
if [[ -n "$SID" ]]; then
  SD=$(curl -s "$BASE/api/snapshot/$SID")
  echo "$SD"|python3 -c "
import json,sys; d=json.load(sys.stdin)['data']
assert d['schemaVersion']=='webui.snapshot.v1','bad schema'
assert d['generatedFrom']['staticAnalysis'] is True,'no static'
e=d.get('explore',{})
ok=len(e.get('symbols',[]))>0 or len(e.get('sourceFiles',[]))>0 or d.get('summary',{}).get('sourceFileCount',0)>0
assert ok,'no data'
ag=d.get('automationGraph',{})
assert ag and (ag.get('summary') or ag.get('status')=='not_collected'),'no automation graph section'
raw=json.dumps(d)
assert '/Users/' not in raw,'path leak'
" 2>/dev/null && pass "detail valid"||fail "detail invalid"
fi

# Error: invalid root
ERR=$(curl -s -X POST "$BASE/api/generate-snapshot" -H "Content-Type: application/json" -d '{"root":"/nonexistent","language":"rust"}')
echo "$ERR"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False and d['error']" 2>/dev/null && pass "invalid root error"||fail "invalid root error"

# Error: unsupported lang
ERR2=$(curl -s -X POST "$BASE/api/generate-snapshot" -H "Content-Type: application/json" -d "{\"root\":\"$FIXTURE\",\"language\":\"julia\"}")
echo "$ERR2"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "bad lang error"||fail "bad lang error"

kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"

T=$((P+F))
echo ""; echo "=== Results: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "RUNNER SMOKE PASSED"
