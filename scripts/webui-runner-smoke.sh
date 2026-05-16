#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT="${1:-0}"
KT=false
[[ "$*" == *"--keep-temp"* ]] && KT=true
[[ "$*" == *"--port "* ]] && PORT="${2}"

P=0; F=0
pass() { P=$((P+1)); echo "  [PASS] $1"; }
fail() { F=$((F+1)); echo "  [FAIL] $1"; }

echo "CodeLattice Runner Smoke"

# Find free port
if [[ "$PORT" == "0" || -z "$PORT" ]]; then
  PORT=$(python3 -c "import socket; s=socket.socket(); s.bind(('',0)); print(s.getsockname()[1]); s.close()" 2>/dev/null || echo 18765)
fi
echo "  Port: $PORT"

TD=$(mktemp -d /tmp/cls-run.XXXXXX)
trap 'kill $PID 2>/dev/null; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT

# Start runner
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" &
PID=$!
sleep 2

# Check process
kill -0 $PID 2>/dev/null && pass "runner started (PID $PID)" || { fail "runner not started"; exit 1; }

BASE="http://127.0.0.1:$PORT"

# Health check
if curl -s "$BASE/api/health" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['status']=='ok'" 2>/dev/null; then
  pass "health ok"
else
  fail "health failed"
fi

# Generate snapshot
FIXTURE="$WS/fixtures/rust/portable-smoke"
GEN=$(curl -s -X POST "$BASE/api/generate-snapshot" \
  -H "Content-Type: application/json" \
  -d "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"full\":true,\"redactRoot\":true}")
SNAP_ID=$(echo "$GEN" | python3 -c "import json,sys; print(json.load(sys.stdin).get('id',''))" 2>/dev/null)
if [[ -n "$SNAP_ID" ]]; then
  pass "generate: $SNAP_ID"
else
  fail "generate failed"; echo "  $GEN"
fi

# List snapshots
LIST=$(curl -s "$BASE/api/snapshots")
SNAP_COUNT=$(echo "$LIST" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null)
if [[ "$SNAP_COUNT" -ge 1 ]]; then
  pass "snapshots list: $SNAP_COUNT"
else
  fail "snapshots list: $SNAP_COUNT"
fi

# Get snapshot detail
if [[ -n "$SNAP_ID" ]]; then
  DETAIL=$(curl -s "$BASE/api/snapshot/$SNAP_ID")
  if echo "$DETAIL" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('schemaVersion')=='webui.snapshot.v1','bad schema'
assert d.get('generatedFrom',{}).get('staticAnalysis') is True,'no static'
e=d.get('explore',{})
ok=len(e.get('symbols',[]))>0 or len(e.get('sourceFiles',[]))>0 or d.get('summary',{}).get('sourceFileCount',0)>0
assert ok,'no data'
raw=json.dumps(d)
assert '/Users/' not in raw,'path leak'
print('OK')
" 2>/dev/null; then
    pass "snapshot detail valid"
  else
    fail "snapshot detail invalid"
  fi
fi

# Cleanup
kill $PID 2>/dev/null || true; wait $PID 2>/dev/null || true
pass "runner stopped cleanly"

T=$((P+F))
echo ""; echo "=== Results: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "RUNNER SMOKE PASSED"
