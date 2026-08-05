#!/usr/bin/env bash
# production WKWebView selftest：不依赖 Vite 端口，只管理本次精确启动的 PID。

set -euo pipefail

WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMEOUT=240
KEEP=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout)
      TIMEOUT="$2"
      shift 2
      ;;
    --keep-running)
      KEEP=true
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

OUT="${CODELATTICE_SMOKE_OUT:-$WS/target/selftest-report.json}"
LOG="${CODELATTICE_SELFTEST_LOG:-$WS/target/tauri-selftest.log}"
BINARY="$WS/apps/desktop/src-tauri/target/release/codelattice-workbench"
SNAP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codelattice-selftest.XXXXXX")"
APP_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    if [[ "$KEEP" != true ]]; then
      kill "$APP_PID" 2>/dev/null || true
      wait "$APP_PID" 2>/dev/null || true
    fi
  fi
  rm -rf "$SNAP_DIR"
}
trap cleanup EXIT

mkdir -p "$WS/target"
cp "$WS/fixtures/webui-snapshots/rust-portable-smoke.snapshot.json" \
  "$SNAP_DIR/rust-portable-smoke.snapshot.json"
rm -f "$OUT" "${OUT%.json}.trace.log"

echo "[selftest] building production frontend and Tauri binary"
(cd "$WS/apps/desktop" && npm run tauri build -- --no-bundle)

EXIT_AFTER_REPORT=1
if [[ "$KEEP" == true ]]; then
  EXIT_AFTER_REPORT=0
fi
echo "[selftest] launching exact binary (timeout ${TIMEOUT}s)"
CODELATTICE_SELFTEST=1 \
CODELATTICE_SELFTEST_EXIT="$EXIT_AFTER_REPORT" \
CODELATTICE_SMOKE_OUT="$OUT" \
CODELATTICE_SNAP_DIR="$SNAP_DIR" \
CODELATTICE_PUBLISH_DIR="$SNAP_DIR/published" \
  "$BINARY" >"$LOG" 2>&1 &
APP_PID=$!
echo "[selftest] appPid=$APP_PID"

START=$SECONDS
while [[ ! -f "$OUT" ]]; do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "[selftest] app exited before writing report" >&2
    tail -30 "$LOG" >&2 || true
    exit 1
  fi
  if (( SECONDS - START >= TIMEOUT )); then
    echo "[selftest] timeout: no report after ${TIMEOUT}s" >&2
    tail -30 "$LOG" >&2 || true
    exit 1
  fi
  sleep 0.25
done

python3 - "$OUT" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
ok = report.get("allPass") is True
print(json.dumps(report, ensure_ascii=False, indent=2))
print("allPass:", ok)
for step in report.get("steps", []):
    mark = "PASS" if step.get("pass") else "FAIL"
    detail = f" — {step.get('detail')}" if not step.get("pass") else ""
    print(f"  [{mark}] {step.get('name')}{detail}")
raise SystemExit(0 if ok else 1)
PY

echo "[selftest] PASS"
