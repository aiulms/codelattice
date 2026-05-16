#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 29766)
TD=$(mktemp -d /tmp/cls-ct.XXXXXX)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "Runner API Contract Test (Phase F)"
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null||{ fail "start"; exit 1; }
pass "start"
B="http://127.0.0.1:$PORT"; FIX="$WS/fixtures/rust/portable-smoke"

# Happy path
curl -s "$B/api/health"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and d['data']['status']=='ok'" 2>/dev/null && pass "GET health"||fail "GET health"
curl -s "$B/api/profiles"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and isinstance(d['data'],list)" 2>/dev/null && pass "GET profiles"||fail "GET profiles"

P1=$(curl -s -X POST "$B/api/profiles" -H "Content-Type: application/json" -d '{"name":"Test","root":"'"$FIX"'","language":"rust"}')
PID1=$(echo "$P1"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];print(d['data']['id'])" 2>/dev/null)
[[ -n "$PID1" ]] && pass "POST profile: $PID1" || { fail "POST profile"; echo "  ${P1:0:200}"; }

curl -s -X PUT "$B/api/profile/$PID1" -H "Content-Type: application/json" -d '{"name":"Updated"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "PUT profile"||fail "PUT profile"

SG=$(curl -s -X POST "$B/api/profile/$PID1/generate-snapshot"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'];print(d['data']['id'])" 2>/dev/null)
[[ -n "$SG" ]] && pass "POST gen-for-profile: $SG" || fail "POST gen-for-profile"

curl -s "$B/api/snapshots"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and len(d['data'])>=1" 2>/dev/null && pass "GET snapshots"||fail "GET snapshots"
curl -s "$B/api/snapshot/$SG"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and d['data']['schemaVersion']=='webui.snapshot.v1'" 2>/dev/null && pass "GET snapshot"||fail "GET snapshot"
curl -s -X POST "$B/api/rebuild-index"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "rebuild-index"||fail "rebuild-index"

# Error paths
curl -s -X POST "$B/api/generate-snapshot" -H "Content-Type: application/json" -d 'notjson'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "invalid JSON body"||fail "invalid JSON body"
curl -s -X POST "$B/api/generate-snapshot" -H "Content-Type: application/json" -d '{"language":"rust"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing root"||fail "missing root"
curl -s -X POST "$B/api/generate-snapshot" -H "Content-Type: application/json" -d '{"root":"/nonexistent","language":"rust"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "root not found"||fail "root not found"
curl -s -X POST "$B/api/generate-snapshot" -H "Content-Type: application/json" -d '{"root":"'"$WS"'/README.md","language":"rust"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "root is file"||fail "root is file"
curl -s -X POST "$B/api/generate-snapshot" -H "Content-Type: application/json" -d '{"root":"'"$FIX"'","language":"julia"}'|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False and d['error']" 2>/dev/null && pass "bad lang"||fail "bad lang"
curl -s --path-as-is "$B/api/snapshot/../bad"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "path traversal"||fail "path traversal"
curl -s "$B/api/snapshot/deadbeef"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing snap"||fail "missing snap"
curl -s -X DELETE "$B/api/snapshot/deadbeef"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "delete missing"||fail "delete missing"
curl -s "$B/api/profile/deadbeef"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "missing prof"||fail "missing prof"

# Delete happy path
curl -s -X DELETE "$B/api/snapshot/$SG"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "DELETE snap"||fail "DELETE snap"
curl -s -X DELETE "$B/api/profile/$PID1"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']" 2>/dev/null && pass "DELETE prof"||fail "DELETE prof"

kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"
T=$((P+F))
echo ""; echo "=== Contract: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "CONTRACT PASSED"
