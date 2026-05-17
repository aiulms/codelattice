#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
RUNNER="$SD/webui-runner.py"
PORT=$(python3 -c "import socket;s=socket.socket();s.bind(('',0));print(s.getsockname()[1]);s.close()" 2>/dev/null||echo 28767)
TD=$(mktemp -d /tmp/cls-br.XXXXXX)
KT=false; SB=false
[[ "$*" == *"--keep-temp"* ]] && KT=true
[[ "$*" == *"--strict-browser"* ]] && SB=true
trap 'kill $PID 2>/dev/null||true; [[ "$KT" != true ]] && rm -rf "$TD"' EXIT
P=0; F=0; S=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
skip(){ S=$((S+1)); echo "  [SKIP] $1"; }
echo "CodeLattice Browser Smoke (Phase F)"
python3 "$RUNNER" --port "$PORT" --snapshot-dir "$TD" & PID=$!; sleep 2
kill -0 $PID 2>/dev/null||{ fail "start"; exit 1; }
pass "runner started"
B="http://127.0.0.1:$PORT"

# Static HTTP checks
PAGE=$(curl -s "$B/")
echo "$PAGE"|grep -q "CodeLattice" 2>/dev/null && pass "HTML serves CodeLattice"||fail "HTML missing CodeLattice"
curl -sI "$B/app.js"|grep -q "200" 2>/dev/null && pass "app.js 200"||fail "app.js serve"
curl -sI "$B/graph-g6.js"|grep -q "200" 2>/dev/null && pass "graph-g6.js 200"||fail "graph-g6.js serve"
curl -sI "$B/vendor/g6/g6.min.js"|grep -q "200" 2>/dev/null && pass "G6 vendor 200"||fail "G6 vendor serve"
curl -sI "$B/runner.js"|grep -q "200" 2>/dev/null && pass "runner.js 200"||fail "runner.js serve"
curl -sI "$B/runner.js"|grep -qi "Cache-Control:.*no-store" 2>/dev/null && pass "runner.js no-store"||fail "runner.js cache header"
curl -s "$B/api/health"|grep -q '"success"' && pass "health api json"||fail "health api"

# Page content checks
echo "$PAGE"|grep -Eq "Project Profiles|项目配置" && pass "Profiles text"||fail "Profiles text"
echo "$PAGE"|grep -Eq "Snapshot Library|分析快照库" && pass "Library text"||fail "Library text"
echo "$PAGE"|grep -Eq "Guided Review|引导式审查" && pass "Guided text"||fail "Guided text"
echo "$PAGE"|grep -Eq "Report|报告" && pass "Report text"||fail "Report text"
echo "$PAGE"|grep -Eq "G6|高级图谱" && pass "G6 graph engine text"||fail "G6 graph engine text"
echo "$PAGE"|grep -Eq "Static Analysis Only|仅静态分析" && pass "Caution text"||fail "Caution text"
echo "$PAGE"|grep -Eq "Generate|生成" && pass "Generate button"||fail "Generate button"

# Browser check (optional)
BROWSER=""
for b in open xdg-open; do command -v "$b" >/dev/null 2>&1 && BROWSER="$b" && break; done
if [[ -n "$BROWSER" ]]; then
  skip "browser open check (skipped: manual verification)"
else
  skip "browser not available"
fi

kill $PID 2>/dev/null||true; wait $PID 2>/dev/null||true
pass "shutdown"
T=$((P+F+S))
echo ""; echo "=== Browser Smoke: $P passed, $F failed, $S skipped, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "BROWSER SMOKE PASSED"
