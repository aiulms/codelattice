#!/usr/bin/env bash
# webui-snapshot.sh — Generate CodeLattice WebUI Snapshot (V1) — Phase A Enriched
#
# Aggregates CLI analysis results into a single JSON snapshot
# conforming to docs/webui/webui-snapshot-contract.md.
#
# Phase A enhancements:
#   - Extracts explore data (symbols + source files) from CLI analyze JSON
#   - Computes heuristic cleanup/release review summaries from graph structure
#   - Embeds 10 workflow presets with tool-chain recommendations
#   - Detects entry points and fan-out hotspots for insights
#   - Supports --redact-root for fixture snapshots (no absolute paths)
#
# Architecture:
#   Bash handles argument parsing, binary discovery, and temp file management.
#   Python script (codelattice-snapshot-gen.py) does all JSON enrichment.
#   No heredoc issues — Python code lives in its own .py file.

set -euo pipefail

ROOT=""
LANGUAGE="auto"
OUTPUT=""
COMPACT=false
INCLUDE_EXPLORE=true
INCLUDE_REVIEW=true
INCLUDE_WORKFLOWS=true
REDACT_ROOT=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
GEN_PY="${SCRIPT_DIR}/codelattice-snapshot-gen.py"

usage() {
  cat <<EOF
Usage: $(basename "$0") --root <path> --language <lang> --output <path> [options]

Generate a CodeLatticeWebSnapshotV1 JSON (Phase A enriched) for WebUI consumption.

Required:
  --root <path>        Project root directory
  --language <lang>    rust|cangjie|arkts|typescript|c|cpp|python|auto
  --output <path>      Output JSON file path, or '-' for stdout

Options:
  --compact            Minify JSON output
  --full               Enable all enrichment [default]
  --include-explore    Extract explore data (source files + symbols) [default: on]
  --include-review     Extract cleanup/release/insight summaries [default: on]
  --include-workflows  Embed workflow preset recommendations [default: on]
  --redact-root        Redact absolute paths to <redacted-root> in output
  --no-enrichment      Skip all Phase A enrichments (minimal snapshot)
  -h, --help           Show this help
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --language) LANGUAGE="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --compact) COMPACT=true; shift ;;
    --full) INCLUDE_EXPLORE=true; INCLUDE_REVIEW=true; INCLUDE_WORKFLOWS=true; shift ;;
    --include-explore) INCLUDE_EXPLORE=true; shift ;;
    --include-review) INCLUDE_REVIEW=true; shift ;;
    --include-workflows) INCLUDE_WORKFLOWS=true; shift ;;
    --redact-root) REDACT_ROOT=true; shift ;;
    --no-enrichment) INCLUDE_EXPLORE=false; INCLUDE_REVIEW=false; INCLUDE_WORKFLOWS=false; shift ;;
    -h|--help) usage ;;
    *) echo "Error: Unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$ROOT" ]]; then echo "Error: --root is required" >&2; exit 1; fi
if [[ ! "$ROOT" = /* ]]; then ROOT="$(cd "$ROOT" 2>/dev/null && pwd)" || { echo "Error: $ROOT not found" >&2; exit 1; }; fi
if [[ ! -d "$ROOT" ]]; then echo "Error: root not a directory: $ROOT" >&2; exit 1; fi
if [[ -z "$OUTPUT" ]]; then echo "Error: --output is required" >&2; exit 1; fi
if [[ "$OUTPUT" != "-" ]] && [[ ! -d "$(dirname "$OUTPUT")" ]]; then echo "Error: output dir not found" >&2; exit 1; fi

# ── Discover codelattice binary ──────────────────────────────────────────────

CODELATTICE=""
for candidate in "${WORKSPACE_ROOT}/target/release/codelattice" "${WORKSPACE_ROOT}/target/debug/codelattice" "$(command -v codelattice 2>/dev/null)"; do
  if [[ -n "${candidate:-}" && -x "${candidate}" ]]; then CODELATTICE="$candidate"; break; fi
done
if [[ -z "$CODELATTICE" ]]; then
  echo "Error: Cannot find codelattice binary. Run 'cargo build --release --bins' first." >&2; exit 1
fi

# ── Validate language ────────────────────────────────────────────────────────

VALID_LANGUAGES=("rust" "cangjie" "arkts" "typescript" "c" "cpp" "python" "auto")
LANG_VALID=0; for vl in "${VALID_LANGUAGES[@]}"; do [[ "$LANGUAGE" == "$vl" ]] && LANG_VALID=1 && break; done
if [[ $LANG_VALID -eq 0 ]]; then echo "Error: unsupported language: $LANGUAGE" >&2; exit 1; fi

# ── Ensure gen.py exists ─────────────────────────────────────────────────────

if [[ ! -f "$GEN_PY" ]]; then
  echo "Error: Missing $GEN_PY. Ensure scripts/codelattice-snapshot-gen.py exists." >&2; exit 1
fi

# ── Run CLI commands & collect output ────────────────────────────────────────

run_analyze() {
  local out; out=$("$CODELATTICE" analyze --root "$ROOT" --language "$LANGUAGE" --format json 2>/dev/null) || true
  if [[ -z "$out" ]] || ! echo "$out" | head -c1 | grep -q '{'; then
    echo '{"_error": "analyze_failed"}'; return 1
  fi
  echo "$out"
}

run_quality() {
  local out; out=$("$CODELATTICE" quality --root "$ROOT" --language "$LANGUAGE" 2>/dev/null) || true
  if [[ -z "$out" ]] || ! echo "$out" | head -c1 | grep -q '{'; then
    echo '{"overall": "unknown", "gates": []}'; return 1
  fi
  echo "$out"
}

# ── Generate snapshot via Python ─────────────────────────────────────────────

generate_snapshot() {
  local analyze_json quality_json timestamp version_str tmp_analyze tmp_quality
  analyze_json=$(run_analyze)
  quality_json=$(run_quality)
  timestamp=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00" 2>/dev/null || date +"%Y-%m-%dT%H:%M:%S%z")
  version_str=$("$CODELATTICE" --version 2>/dev/null | head -1 || echo "unknown")

  # Write to temp files (avoids shell quoting/heredoc issues entirely)
  tmp_analyze=$(mktemp /tmp/codelattice-snap-analyze.XXXXXX.json)
  tmp_quality=$(mktemp /tmp/codelattice-snap-quality.XXXXXX.json)

  printf '%s\n' "$analyze_json" > "$tmp_analyze"
  printf '%s\n' "$quality_json" > "$tmp_quality"

  # Build flag list for python
  local py_extra=()
  $INCLUDE_EXPLORE && py_extra+=(EXPLORE)
  $INCLUDE_REVIEW && py_extra+=(REVIEW)
  $INCLUDE_WORKFLOWS && py_extra+=(WORKFLOWS)
  $REDACT_ROOT && py_extra+=(REDACT)
  $COMPACT && py_extra+=(COMPACT)

  python3 "$GEN_PY" \
    "$tmp_analyze" "$tmp_quality" \
    "$timestamp" "$version_str" \
    "$ROOT" "$LANGUAGE" \
    "${py_extra[@]+"${py_extra[@]}"}"

  local rc=$?
  rm -f "$tmp_analyze" "$tmp_quality"
  return $rc
}

# ── Output ───────────────────────────────────────────────────────────────────

if [[ "$OUTPUT" == "-" ]]; then
  generate_snapshot
else
  generate_snapshot > "$OUTPUT"
fi

echo "[snapshot] Generated: $OUTPUT ($(wc -c < "$OUTPUT" 2>/dev/null || echo '?') bytes)" >&2
