#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 29768)
TD=$(mktemp -d /tmp/cls-mcp.XXXXXX)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "Live MCP Smoke (Phase G)"
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD/snaps" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null||{ fail "start"; exit 1; }
pass "runner started"
B="http://127.0.0.1:$PORT"; FIX="$WS/fixtures/rust/portable-smoke"

# MCP status (may need warmup)
sleep 1
curl -s "$B/api/mcp/status"|python3 -c "import json,sys;d=json.load(sys.stdin);print(d.get('success'))" >/dev/null 2>&1
ST_OK=$?; [[ $ST_OK -eq 0 ]] && pass "mcp status"||fail "mcp status"

# MCP tools
TOOLS=$(curl -s "$B/api/mcp/tools")
TC=$(echo "$TOOLS"|python3 -c "import json,sys;d=json.load(sys.stdin);print(len(d['data']))" 2>/dev/null||echo 0)
[[ "$TC" -gt 0 ]] && pass "mcp tools: $TC" || { fail "mcp tools"; echo "  count=$TC"; }

# Create job: project_overview
JOB=$(curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d "{\"root\":\"$FIX\",\"language\":\"rust\",\"workflow\":\"project_overview\"}")
JID=$(echo "$JOB"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
[[ -n "$JID" ]] && pass "create job: $JID" || { fail "create job"; echo "  ${JOB:0:200}"; }

# Poll job
for i in $(seq 1 30); do
  ST=$(curl -s "$B/api/mcp/job/$JID"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['status'])" 2>/dev/null)
  if [[ "$ST" == "succeeded" || "$ST" == "failed" ]]; then break; fi; sleep 1
done
[[ "$ST" == "succeeded" ]] && pass "job succeeded" || { fail "job $ST"; echo "  checking..."; }

# List jobs
LJ=$(curl -s "$B/api/mcp/jobs")
JC=$(echo "$LJ"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$JC" -ge 1 ]] && pass "jobs list: $JC" || fail "jobs list"

# Error: unsupported workflow
ERR=$(curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d "{\"root\":\"$FIX\",\"language\":\"rust\",\"workflow\":\"nonexistent\"}")
echo "$ERR"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "bad workflow"||fail "bad workflow"

# Error: missing root
ERR2=$(curl -s -X POST "$B/api/mcp/jobs" -H "Content-Type: application/json" -d '{"language":"rust","workflow":"project_overview"}')
echo "$ERR2"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing root"||fail "missing root"

# Cancel
CAN=$(curl -s -X POST "$B/api/mcp/job/$JID/cancel"|python3 -c "import json,sys;d=json.load(sys.stdin);print(d.get('success'))" 2>/dev/null)
[[ "$CAN" == "True" || "$CAN" == "False" ]] && pass "cancel response"||fail "cancel"

# Delete
DEL=$(curl -s -X DELETE "$B/api/mcp/job/$JID"|python3 -c "import json,sys;d=json.load(sys.stdin);print(d.get('success'))" 2>/dev/null)
[[ "$DEL" == "True" ]] && pass "delete job"||fail "delete"

kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"
T=$((P+F))
echo ""; echo "=== Live MCP: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "LIVE MCP PASSED"
