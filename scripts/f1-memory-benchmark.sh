#!/usr/bin/env bash
# 真实 F1 内存基准：production Tauri binary + 精确 owned PID RSS 采样。

set -euo pipefail

WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMEOUT="${F1_TIMEOUT_SECONDS:-240}"
INTERVAL="${F1_SAMPLE_INTERVAL_SECONDS:-0.20}"
REPORT="${F1_REPORT:-$WS/target/f1-memory-benchmark.json}"
SELFTEST_REPORT="$(mktemp "${TMPDIR:-/tmp}/codelattice-f1-selftest.XXXXXX.json")"
SAMPLES="$(mktemp "${TMPDIR:-/tmp}/codelattice-f1-samples.XXXXXX.ndjson")"
WEBKIT_BASELINE="$(mktemp "${TMPDIR:-/tmp}/codelattice-f1-webkit.XXXXXX.json")"
SNAP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codelattice-f1-snapshots.XXXXXX")"
LOG="$WS/target/f1-memory-benchmark.log"
BINARY="$WS/apps/desktop/src-tauri/target/release/codelattice-workbench"
APP_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  rm -f "$SELFTEST_REPORT" "${SELFTEST_REPORT%.json}.trace.log" "$SAMPLES" "$WEBKIT_BASELINE"
  rm -rf "$SNAP_DIR"
}
trap cleanup EXIT

rm -f "${SELFTEST_REPORT%.json}.trace.log"

mkdir -p "$WS/target"
cp "$WS/fixtures/webui-snapshots/rust-portable-smoke.snapshot.json" \
  "$SNAP_DIR/rust-portable-smoke.snapshot.json"

echo "[f1] building production frontend and Tauri binary"
(cd "$WS/apps/desktop" && npm run tauri build -- --no-bundle)

echo "[f1] launching controlled application"
python3 "$WS/scripts/webui-rss-sampler.py" --list-webkit-pids >"$WEBKIT_BASELINE"
CODELATTICE_SELFTEST=1 \
CODELATTICE_SELFTEST_EXIT=1 \
CODELATTICE_SMOKE_OUT="$SELFTEST_REPORT" \
CODELATTICE_SNAP_DIR="$SNAP_DIR" \
CODELATTICE_PUBLISH_DIR="$SNAP_DIR/published" \
  "$BINARY" >"$LOG" 2>&1 &
APP_PID=$!
echo "[f1] appPid=$APP_PID"

START=$SECONDS
while kill -0 "$APP_PID" 2>/dev/null; do
  if SAMPLE="$(python3 "$WS/scripts/webui-rss-sampler.py" \
      --pid "$APP_PID" \
      --webkit-baseline-file "$WEBKIT_BASELINE" 2>/dev/null)"; then
    printf '%s\n' "$SAMPLE" >>"$SAMPLES"
  fi
  if (( SECONDS - START >= TIMEOUT )); then
    echo "[f1] timeout after ${TIMEOUT}s" >&2
    tail -30 "$LOG" >&2 || true
    exit 1
  fi
  sleep "$INTERVAL"
done
wait "$APP_PID" || {
  echo "[f1] application exited non-zero" >&2
  tail -30 "$LOG" >&2 || true
  exit 1
}
APP_PID=""

if [[ ! -s "$SELFTEST_REPORT" ]]; then
  echo "[f1] missing selftest report" >&2
  exit 1
fi
if [[ ! -s "$SAMPLES" ]]; then
  echo "[f1] no owned-process RSS samples" >&2
  exit 1
fi

python3 - "$SAMPLES" "$SELFTEST_REPORT" "$REPORT" <<'PY'
import datetime
import json
import math
import pathlib
import statistics
import sys

samples_path, selftest_path, report_path = map(pathlib.Path, sys.argv[1:])
samples = [json.loads(line) for line in samples_path.read_text(encoding="utf-8").splitlines() if line]
selftest = json.loads(selftest_path.read_text(encoding="utf-8"))
aggregate = [float(sample["aggregateMb"]) for sample in samples]
verified_samples = [sample for sample in samples if sample.get("ownershipVerified")]
verified_aggregate = [float(sample["aggregateMb"]) for sample in verified_samples]

def percentile(values, fraction):
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * fraction) - 1))
    return ordered[index]

peak = max(aggregate)
final = verified_aggregate[-1] if verified_aggregate else aggregate[-1]
thresholds = {
    "minimumSamples": 3,
    "minimumOwnershipVerifiedSamples": 3,
    "peakAggregateMb": 512.0,
    "finalAggregateMb": 384.0,
}
checks = {
    "selftestPass": selftest.get("allPass") is True,
    "enoughSamples": len(samples) >= thresholds["minimumSamples"],
    "webviewOwnershipVerified": len(verified_samples) >= thresholds["minimumOwnershipVerifiedSamples"],
    "peakWithinLimit": peak <= thresholds["peakAggregateMb"],
    "finalWithinLimit": final <= thresholds["finalAggregateMb"],
}
report = {
    "schemaVersion": "codelattice.f1-memory.v2",
    "generatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "measurement": "macOS RSS of exact app PID/descendants plus pre-launch WebKit PID delta verified by bundle-specific open-file anchor",
    "rootPid": samples[0]["rootPid"],
    "sampleCount": len(samples),
    "ownershipVerifiedSampleCount": len(verified_samples),
    "aggregateMb": {
        "first": aggregate[0],
        "median": round(statistics.median(aggregate), 1),
        "p95": round(percentile(aggregate, 0.95), 1),
        "peak": peak,
        "finalVerified": final,
    },
    "peakCoreMb": max(float(sample["coreMb"]) for sample in samples),
    "peakWebviewMb": max(float(sample["webviewMb"]) for sample in samples),
    "ownedPidListsAtPeak": next(
        {
            "core": sample["corePidList"],
            "webview": sample["webviewPidList"],
        }
        for sample in samples
        if float(sample["aggregateMb"]) == peak
    ),
    "thresholds": thresholds,
    "checks": checks,
    "selftest": selftest,
    "verdict": "pass" if all(checks.values()) else "fail",
}
report_path.parent.mkdir(parents=True, exist_ok=True)
report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
raise SystemExit(0 if report["verdict"] == "pass" else 1)
PY

echo "[f1] report=$REPORT"
