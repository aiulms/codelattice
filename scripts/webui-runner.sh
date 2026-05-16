#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
PORT=8765
SNAPDIR=""
OPEN=false

usage() {
  cat <<EOF
CodeLattice WebUI Local Runner

Usage: $(basename "$0") [options]

Options:
  --port PORT           Listen port (default: 8765)
  --snapshot-dir PATH   Snapshot library directory (default: .codelattice-webui/snapshots/)
  --open                Open browser after start
  -h, --help            Show this help

The runner starts a local HTTP server on 127.0.0.1 and serves the
webui/snapshot-viewer/ directory, with a REST API for generating
and managing snapshots.

Security: binds 127.0.0.1 only. Does not write to target projects.
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --snapshot-dir) SNAPDIR="$2"; shift 2 ;;
    --open) OPEN=true; shift ;;
    -h|--help) usage ;;
    *) echo "Unknown: $1"; usage ;;
  esac
done

PYTHON_BIN=""
for py in python3 python; do
  if command -v "$py" >/dev/null 2>&1; then PYTHON_BIN="$py"; break; fi
done
if [[ -z "$PYTHON_BIN" ]]; then echo "Error: python3 not found"; exit 1; fi

RUNNER="$SD/webui-runner.py"
[[ -f "$RUNNER" ]] || { echo "Error: webui-runner.py not found at $RUNNER"; exit 1; }

ARGS=(--port "$PORT")
[[ -n "$SNAPDIR" ]] && ARGS+=(--snapshot-dir "$SNAPDIR")

echo "Starting CodeLattice WebUI Runner..."
echo "  Port: $PORT"
echo "  Python: $PYTHON_BIN"
[[ -n "$SNAPDIR" ]] && echo "  Snapshot dir: $SNAPDIR"

if [[ "$OPEN" == true ]]; then
  echo "  Opening browser..."
  (sleep 1 && open "http://127.0.0.1:$PORT" 2>/dev/null || xdg-open "http://127.0.0.1:$PORT" 2>/dev/null || echo "  Please open http://127.0.0.1:$PORT") &
fi

exec "$PYTHON_BIN" "$RUNNER" "${ARGS[@]}"
