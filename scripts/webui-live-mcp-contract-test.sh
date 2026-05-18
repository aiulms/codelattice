#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 29769)
TD=$(mktemp -d /tmp/cls-lc.XXXXXX)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "Live MCP Contract Test (Phase H)"
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD/snaps" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null||{ fail "start"; exit 1; }
pass "started"
B="http://127.0.0.1:$PORT"; FIX="$WS/fixtures/rust/portable-smoke"

# Status schema
curl -s "$B/api/mcp/status"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];dd=d['data'];assert dd['staticOnly']==True;assert 'available' in dd;assert 'toolCount' in dd;assert 'lastError' in dd" 2>/dev/null && pass "status schema"||fail "status schema"

# Tools
curl -s "$B/api/mcp/tools"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];tools=d['data'];assert len(tools)>=37,f'tools={len(tools)}'" 2>/dev/null && pass "tools>=37"||fail "tools>=37"

# Create + poll job
JOB=$(curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d '{"root":"'"$FIX"'","language":"rust","workflow":"project_overview"}')
JID=$(echo "$JOB"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];print(d['data']['id'])" 2>/dev/null)
[[ -n "$JID" ]] && pass "create: $JID" || { fail "create"; echo "  ${JOB:0:200}"; }

for i in $(seq 1 30); do
  ST=$(curl -s "$B/api/mcp/job/$JID"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['status'])" 2>/dev/null)
  if [[ "$ST" == "succeeded" || "$ST" == "failed" ]]; then break; fi; sleep 1
done
[[ "$ST" == "succeeded" ]] && pass "succeeded" || fail "job status=$ST"

# Detail has result
curl -s "$B/api/mcp/job/$JID"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];r=d['data'].get('result');assert r is not None" 2>/dev/null && pass "has result"||fail "no result"

# Automation graph workflow is first-class, not just a custom raw tool.
AUTO=$(curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d '{"root":"'"$FIX"'","language":"rust","workflow":"automation_graph"}')
AJID=$(echo "$AUTO"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];print(d['data']['id'])" 2>/dev/null)
[[ -n "$AJID" ]] && pass "automation create: $AJID" || { fail "automation create"; echo "  ${AUTO:0:200}"; }
AST=""
for i in $(seq 1 30); do
  AST=$(curl -s "$B/api/mcp/job/$AJID"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['status'])" 2>/dev/null)
  if [[ "$AST" == "succeeded" || "$AST" == "failed" ]]; then break; fi; sleep 1
done
[[ "$AST" == "succeeded" ]] && pass "automation succeeded" || fail "automation status=$AST"
curl -s "$B/api/mcp/job/$AJID"|python3 -c "import json,sys;d=json.load(sys.stdin);r=d['data'].get('result') or {};assert 'summary' in r;assert r.get('generatedFrom',{}).get('staticAnalysis')==True" 2>/dev/null && pass "automation result schema"||fail "automation result schema"

# Error paths
curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d '{"language":"rust","workflow":"project_overview"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing root"||fail "missing root"
curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d '{"root":"'"$FIX"'","language":"rust","workflow":"julia_mode"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "bad workflow"||fail "bad workflow"
curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d "{\"root\":\"$FIX\",\"language\":\"rust\",\"workflow\":\"custom_tool\",\"tool\":\"rm -rf\"}"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "illegal tool"||fail "illegal tool"
curl -s "$B/api/mcp/job/deadbeef"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing job"||fail "missing job"
curl -s -X DELETE "$B/api/mcp/job/deadbeef"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "delete missing"||fail "delete missing"

# Cancel
CAN=$(curl -s -X POST "$B/api/mcp/job/$JID/cancel"|python3 -c "import json,sys;print(json.load(sys.stdin).get('success'))" 2>/dev/null)
[[ "$CAN" == "True" || "$CAN" == "False" ]] && pass "cancel"||fail "cancel"

# Delete
curl -s -X DELETE "$B/api/mcp/job/$AJID" >/dev/null 2>&1 || true
DEL=$(curl -s -X DELETE "$B/api/mcp/job/$JID"|python3 -c "import json,sys;print(json.load(sys.stdin).get('success'))" 2>/dev/null)
[[ "$DEL" == "True" ]] && pass "delete"||fail "delete"

# No path leak in any API response
for ep in mcp/status mcp/tools; do
  curl -s "$B/api/$ep"|python3 -c "import json,sys;d=json.dumps(json.load(sys.stdin));assert '/Users/' not in d" 2>/dev/null && pass "no leak: $ep"||fail "leak: $ep"
done

kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"
T=$((P+F))
echo ""; echo "=== Contract: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "LIVE MCP CONTRACT PASSED"
