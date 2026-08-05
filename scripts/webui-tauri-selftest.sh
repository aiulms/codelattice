#!/usr/bin/env bash
# webui-tauri-selftest.sh — P0-F1 #7 / G1 gate：真实 WKWebView 内的 G6 smoke。
#
# 执行：CODELATTICE_SELFTEST=1 启动 tauri dev → selftest 在 WebView 内跑
# （mount / node click / edge click / resize / 重复 mount/unmount ×3）→
# 结果写 CODELATTICE_SMOKE_OUT → 本脚本等待文件出现后退出。
#
# 用法：
#   bash scripts/webui-tauri-selftest.sh [--timeout 秒] [--keep-running]

set -u
WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMEOUT=240
KEEP=false
for a in "$@"; do
  case "$a" in
    --timeout) TIMEOUT="$2"; shift 2 ;;
    --keep-running) KEEP=true ;;
  esac
done

OUT="${CODELATTICE_SMOKE_OUT:-$WS/target/selftest-report.json}"
rm -f "$OUT"

echo "[selftest] starting tauri dev with CODELATTICE_SELFTEST=1 (timeout ${TIMEOUT}s)"
cd "$WS/apps/desktop" || exit 1
CODELATTICE_SELFTEST=1 CODELATTICE_SMOKE_OUT="$OUT" npx tauri dev > /tmp/tauri-selftest.log 2>&1 &
PID=$!
trap '[[ "$KEEP" != true ]] && kill $PID 2>/dev/null; pkill -f "codelattice-workbench" 2>/dev/null; true' EXIT

# 等待报告文件
ELAPSED=0
while [[ ! -f "$OUT" ]]; do
  sleep 3
  ELAPSED=$((ELAPSED + 3))
  if [[ $ELAPSED -ge $TIMEOUT ]]; then
    echo "[selftest] TIMEOUT after ${TIMEOUT}s — no report file"
    tail -20 /tmp/tauri-selftest.log
    exit 1
  fi
done

sleep 1
echo "[selftest] report written after ${ELAPSED}s:"
cat "$OUT"
echo ""
python3 - "$OUT" << 'PYEOF'
import json, sys
report = json.load(open(sys.argv[1]))
ok = report.get("allPass") is True
print("allPass:", ok)
for s in report.get("steps", []):
    mark = "PASS" if s.get("pass") else "FAIL"
    print(f"  [{mark}] {s.get('name')}" + (f" — {s.get('detail')}" if not s.get("pass") else ""))
sys.exit(0 if ok else 1)
PYEOF
RC=$?
echo "[selftest] exit=$RC"
exit $RC
