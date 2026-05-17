#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
run_test(){ echo ""; echo "=== $1 ==="; bash "$SD/$1" 2>&1|tail -3; }
echo "CodeLattice Beta Sanity (Phase F)"
echo ""
echo "--- Static Checks ---"
# .gitignore
grep -q ".codelattice-webui" "$WS/.gitignore" && pass ".codelattice-webui gitignored"||fail "gitignore"
# Runner host check
grep -q '"127.0.0.1"' "$SD/webui-runner.py" && pass "runner binds 127.0.0.1"||fail "runner host"
! grep -q "0.0.0.0" "$SD/webui-runner.py" 2>/dev/null || { fail "runner has 0.0.0.0"; }
# subprocess safety
grep -q "subprocess.run" "$SD/webui-runner.py" && pass "runner uses subprocess.run"||fail "subprocess usage"
! grep -q "shell=True" "$SD/webui-runner.py" 2>/dev/null && pass "no shell=True"||fail "shell=True found"
# fixture path leaks
for f in "$WS/fixtures/webui-snapshots/"*.json; do
  python3 -c "import json;d=json.load(open('$f'));raw=json.dumps(d);assert '/Users/' not in raw" 2>/dev/null && pass "$(basename $f) no leak"||fail "$(basename $f) leak"
done
# no npm
for f in package.json pnpm-lock.yaml yarn.lock; do
  [[ ! -f "$WS/$f" ]] && pass "no $f"||fail "$f exists"
done
echo ""; echo "--- Running Tests ---"
run_test "webui-snapshot-smoke.sh" --full
run_test "webui-viewer-smoke.sh" --skip-browser
run_test "webui-runner-smoke.sh"
run_test "webui-runner-contract-test.sh"
run_test "webui-workbench-trial.sh"
run_test "webui-browser-smoke.sh"
run_test "webui-live-mcp-smoke.sh"
run_test "webui-live-mcp-contract-test.sh"
echo ""; echo "=== Beta Sanity: check log above ==="
