#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 29765)
KT=false; [[ "$*" == *"--keep-temp"* ]] && KT=true
TD=$(mktemp -d /tmp/cls-trial.XXXXXX)
trap 'kill $PID 2>/dev/null || true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "CodeLattice Workbench Trial (Phase E)"
echo "  Port: $PORT  Temp: $TD"

# Start runner
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD/snaps" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null || { fail "runner not started"; exit 1; }
pass "runner started"
BASE="http://127.0.0.1:$PORT"

# Health
curl -s "$BASE/api/health"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success'] and d['data']['status']=='ok'" 2>/dev/null && pass "health"||fail "health"

# Project inventory: supported root
QROOT=$(python3 -c 'import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))' "$WS/fixtures/rust/portable-smoke")
INV=$(curl -s "$BASE/api/project/inventory?root=$QROOT")
echo "$INV"|python3 -c "import json,sys;d=json.load(sys.stdin)['data'];assert d['status']=='root_project' and 'rust' in d['supportedLanguages']" 2>/dev/null && pass "inventory supported root" || fail "inventory supported root"

# Project inventory: multi-project root + unsupported module
MR="$TD/multi-root"; mkdir -p "$MR/rustlib" "$MR/tsapp" "$MR/csharp"
printf '[package]\nname=\"demo\"\nversion=\"0.1.0\"\nedition=\"2021\"\n' > "$MR/rustlib/Cargo.toml"
printf '{"compilerOptions":{}}\n' > "$MR/tsapp/tsconfig.json"
printf '<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n' > "$MR/csharp/demo.csproj"
QMR=$(python3 -c 'import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))' "$MR")
MINV=$(curl -s "$BASE/api/project/inventory?root=$QMR")
echo "$MINV"|python3 -c "import json,sys;d=json.load(sys.stdin)['data'];assert d['status']=='multi_project' and d['supportedCandidateCount']>=2 and d['unsupportedCandidateCount']>=1" 2>/dev/null && pass "inventory multi-project" || { fail "inventory multi-project"; echo "  resp: ${MINV:0:300}"; }

# Auto analyze should stop before guessing a multi-project parent
MERR=$(curl -s -X POST "$BASE/api/quick-analyze" -H "Content-Type: application/json" -d "{\"root\":\"$MR\",\"language\":\"auto\"}")
echo "$MERR"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False and '可分析' in (d.get('hint') or '')" 2>/dev/null && pass "multi-project analyze asks candidate" || fail "multi-project analyze asks candidate"

# Create profiles
P1=$(curl -s -X POST "$BASE/api/profiles" -H "Content-Type: application/json" -d "{\"name\":\"Rust Fixture\",\"root\":\"$WS/fixtures/rust/portable-smoke\",\"language\":\"rust\"}")
PF1_ID=$(echo "$P1"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
[[ -n "$PF1_ID" ]] && pass "create profile 1: $PF1_ID" || { fail "create profile 1"; echo "  resp: ${P1:0:200}"; }

P2=$(curl -s -X POST "$BASE/api/profiles" -H "Content-Type: application/json" -d "{\"name\":\"TS Fixture\",\"root\":\"$WS/fixtures/typescript/portable-smoke\",\"language\":\"typescript\"}")
PF2_ID=$(echo "$P2"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
[[ -n "$PF2_ID" ]] && pass "create profile 2: $PF2_ID" || fail "create profile 2"

# List profiles
PL=$(curl -s "$BASE/api/profiles")
PC=$(echo "$PL"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$PC" -ge 2 ]] && pass "profiles list: $PC" || fail "profiles list: $PC"

# Generate snapshot for profile 1
SG=$(curl -s -X POST "$BASE/api/profile/$PF1_ID/generate-snapshot")
SID=$(echo "$SG"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['id'])" 2>/dev/null)
[[ -n "$SID" ]] && pass "gen for profile: $SID" || { fail "gen for profile"; echo "  resp: ${SG:0:200}"; }

# List snapshots
SL=$(curl -s "$BASE/api/snapshots")
SC=$(echo "$SL"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$SC" -ge 1 ]] && pass "snapshots: $SC" || fail "snapshots: $SC"

# Filter by profile
SLP=$(curl -s "$BASE/api/snapshots?profileId=$PF1_ID")
SCP=$(echo "$SLP"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$SCP" -ge 1 ]] && pass "filter by profile: $SCP" || fail "filter by profile: $SCP"

# Filter by language
SLL=$(curl -s "$BASE/api/snapshots?language=rust")
SCL=$(echo "$SLL"|python3 -c "import json,sys;print(len(json.load(sys.stdin)['data']))" 2>/dev/null)
[[ "$SCL" -ge 1 ]] && pass "filter by lang: $SCL" || fail "filter by lang: $SCL"

# Profile lastSnapshotID updated
PF1_UPD=$(curl -s "$BASE/api/profile/$PF1_ID")
PF1_LSID=$(echo "$PF1_UPD"|python3 -c "import json,sys;print(json.load(sys.stdin)['data'].get('lastSnapshotId',''))" 2>/dev/null)
[[ -n "$PF1_LSID" ]] && pass "profile lastSnapshot: $PF1_LSID" || fail "profile lastSnapshot"

# Snapshot detail
SD=$(curl -s "$BASE/api/snapshot/$SID")
echo "$SD"|python3 -c "
import json,sys; d=json.load(sys.stdin)['data']
assert d['schemaVersion']=='webui.snapshot.v1','bad schema'
assert d['generatedFrom']['staticAnalysis'] is True,'no static'
raw=json.dumps(d)
assert '/Users/' not in raw,'path leak'
" 2>/dev/null && pass "snapshot valid" || fail "snapshot invalid"

# Rebuild index
RI=$(curl -s -X POST "$BASE/api/rebuild-index")
RC=$(echo "$RI"|python3 -c "import json,sys;print(json.load(sys.stdin)['data']['rebuilt'])" 2>/dev/null)
[[ "$RC" -ge 1 ]] && pass "rebuild index: $RC" || fail "rebuild index: $RC"

# Error case: invalid root
ERR=$(curl -s -X POST "$BASE/api/generate-snapshot" -H "Content-Type: application/json" -d '{"root":"/nonexistent_path","language":"rust"}')
echo "$ERR"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "invalid root error"||fail "invalid root error"

# Error case: unsupported language
ERR2=$(curl -s -X POST "$BASE/api/generate-snapshot" -H "Content-Type: application/json" -d "{\"root\":\"$WS/fixtures/rust/portable-smoke\",\"language\":\"julia\"}")
echo "$ERR2"|python3 -c "import json,sys;d=json.load(sys.stdin);assert d['success']==False" 2>/dev/null && pass "unsupported lang error"||fail "unsupported lang error"

# Cleanup
kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "clean shutdown"

T=$((P+F))
echo ""; echo "=== Trial Results: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && echo "TRIAL FAILED" && exit 1
echo "TRIAL PASSED"
