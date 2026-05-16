#!/usr/bin/env bash
# webui-snapshot-smoke.sh — Smoke test for CodeLattice WebUI Snapshot V1
#
# Generates Rust and TypeScript fixture snapshots, then validates:
#   - JSON parse OK
#   - schemaVersion == "webui.snapshot.v1"
#   - generatedFrom.staticAnalysis == true
#   - generatedFrom.runtimeVerified == false
#   - summary.sourceFileCount > 0 (or equivalent)
#   - quality section exists
#   - limitations section exists
#
# Usage:
#   bash scripts/webui-snapshot-smoke.sh              # run + cleanup temp
#   bash scripts/webui-snapshot-smoke.sh --keep-temp  # keep temp files for inspection
#
# Exit codes:
#   0 — all checks passed
#   1 — one or more checks failed

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SNAPSHOT_SCRIPT="${SCRIPT_DIR}/webui-snapshot.sh"
KEEP_TEMP=false
PASS=0
FAIL=0
TEMP_DIR=""

# ── Parse args ─────────────────────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --keep-temp) KEEP_TEMP=true ;;
    *)
      echo "Error: Unknown argument: $arg" >&2
      echo "Usage: $(basename "$0") [--keep-temp]" >&2
      exit 1
      ;;
  esac
done

# ── Colors (if terminal) ────────────────────────────────────────────
if [[ -t 1 ]]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  BOLD='\033[1m'
  RESET='\033[0m'
else
  RED='' GREEN='' YELLOW='' BOLD='' RESET=''
fi

pass() { echo -e "  ${GREEN}PASS${RESET}: $1"; ((PASS++)) || true; }
fail() { echo -e "  ${RED}FAIL${RESET}: $1"; ((FAIL++)) || true; }
section() { echo -e "\n${BOLD}$1${RESET}"; }

# ── Setup temp dir ─────────────────────────────────────────────────
TEMP_DIR="$(mktemp -d /tmp/codelattice-webui-smoke.XXXXXX)"
trap 'if [[ "$KEEP_TEMP" != true && -n "$TEMP_DIR" && -d "$TEMP_DIR" ]]; then rm -rf "$TEMP_DIR"; fi' EXIT

echo "CodeLattice WebUI Snapshot Smoke Test"
echo "Temp dir: ${TEMP_DIR}"
if [[ "$KEEP_TEMP" == true ]]; then
  echo "(--keep-temp: temp files will be preserved)"
fi

# ════════════════════════════════════════════════════════════════════
section "Prerequisites"
# ════════════════════════════════════════════════════════════════════

if [[ ! -x "$SNAPSHOT_SCRIPT" ]]; then
  fail "webui-snapshot.sh not found or not executable: $SNAPSHOT_SCRIPT"
  exit 1
fi
pass "webui-snapshot.sh exists"

# Check for codelattice binary
CODELATTICE=""
for candidate in \
  "${WORKSPACE_ROOT}/target/release/codelattice" \
  "${WORKSPACE_ROOT}/target/debug/codelattice" \
  "$(command -v codelattice 2>/dev/null)"; do
  if [[ -n "${candidate:-}" && -x "${candidate}" ]]; then
    CODELATTICE="$candidate"
    break
  fi
done
if [[ -z "$CODELATTICE" ]]; then
  fail "codelattice binary not found. Run 'cargo build --release --bins' first."
  exit 1
fi
pass "codelattice binary found: $CODELATTICE"

# Check python3
if ! command -v python3 &>/dev/null; then
  fail "python3 is required but not found"
  exit 1
fi
pass "python3 available: $(python3 --version 2>&1)"

# ════════════════════════════════════════════════════════════════════
section "Generate Rust snapshot"
# ════════════════════════════════════════════════════════════════════

RUST_OUT="${TEMP_DIR}/rust-portable-smoke.snapshot.json"
bash "$SNAPSHOT_SCRIPT" \
  --root "${WORKSPACE_ROOT}/fixtures/rust/portable-smoke" \
  --language rust \
  --output "$RUST_OUT" 2>&1 || {
  fail "Failed to generate Rust snapshot"
}

if [[ -f "$RUST_OUT" ]]; then
  RUST_SIZE=$(wc -c < "$RUST_OUT" | tr -d ' ')
  pass "Rust snapshot generated (${RUST_SIZE} bytes)"
else
  fail "Rust snapshot file not created at $RUST_OUT"
fi

# ════════════════════════════════════════════════════════════════════
section "Generate TypeScript snapshot"
# ════════════════════════════════════════════════════════════════════

TS_OUT="${TEMP_DIR}/typescript-portable-smoke.snapshot.json"
bash "$SNAPSHOT_SCRIPT" \
  --root "${WORKSPACE_ROOT}/fixtures/typescript/portable-smoke" \
  --language typescript \
  --output "$TS_OUT" 2>&1 || {
  fail "Failed to generate TypeScript snapshot"
}

if [[ -f "$TS_OUT" ]]; then
  TS_SIZE=$(wc -c < "$TS_OUT" | tr -d ' ')
  pass "TypeScript snapshot generated (${TS_SIZE} bytes)"
else
  fail "TypeScript snapshot file not created at $TS_OUT"
fi

# ════════════════════════════════════════════════════════════════════
section "Validate snapshots"
# ════════════════════════════════════════════════════════════════════

validate_snapshot() {
  local label="$1"
  local filepath="$2"

  # Check JSON parse
  local parsed
  parsed=$(python3 -c "
import sys, json
try:
    d = json.load(open('$filepath'))
    print(json.dumps(d))
except Exception as e:
    print(f'PARSE_ERROR: {e}')
    sys.exit(1)
" 2>/dev/null)

  if [[ $? -ne 0 || "$parsed" == PARSE_ERROR* ]]; then
    fail "[$label] JSON parse failed"
    return 1
  fi
  pass "[$label] JSON parse OK"

  # Extract fields via Python for reliable checking
  python3 <<PYEOF
import json, sys

with open('$filepath') as f:
    d = json.load(f)

errors = []

# schemaVersion
sv = d.get('schemaVersion', '')
if sv == 'webui.snapshot.v1':
    print(f"  PASS: [$label] schemaVersion == 'webui.snapshot.v1'")
elif sv:
    print(f"  FAIL: [$label] schemaVersion = '{sv}' (expected 'webui.snapshot.v1')")
    errors.append(1)
else:
    print(f"  FAIL: [$label] schemaVersion missing")
    errors.append(1)

# generatedFrom
gf = d.get('generatedFrom', {})
if gf.get('staticAnalysis') is True:
    print(f"  PASS: [$label] generatedFrom.staticAnalysis == true")
else:
    print(f"  FAIL: [$label] generatedFrom.staticAnalysis != true (got {gf.get('staticAnalysis')})")
    errors.append(1)

if gf.get('runtimeVerified') is False:
    print(f"  PASS: [$label] generatedFrom.runtimeVerified == false")
else:
    print(f"  FAIL: [$label] generatedFrom.runtimeVerified != false (got {gf.get('runtimeVerified')})")
    errors.append(1)

# summary.sourceFileCount > 0
s = d.get('summary', {})
sfc = s.get('sourceFileCount', 0)
if isinstance(sfc, int) and sfc > 0:
    print(f"  PASS: [$label] summary.sourceFileCount = {sfc} (> 0)")
elif sfc == 0:
    # For very small fixtures, sourceFileCount might legitimately be low
    sym_count = s.get('symbolCount', 0)
    if isinstance(sym_count, int) and sym_count > 0:
        print(f"  WARN: [$label] summary.sourceFileCount = 0, but symbolCount = {sym_count}")
        print(f"  PASS: [$label] summary has data (symbolCount={sym_count})")
    else:
        print(f"  FAIL: [$label] summary.sourceFileCount = 0 and no symbols")
        errors.append(1)
else:
    print(f"  FAIL: [$label] summary.sourceFileCount invalid: {sfc}")
    errors.append(1)

# quality section exists
q = d.get('quality', {})
if q and isinstance(q, dict):
    overall = q.get('overall', '(missing)')
    print(f"  PASS: [$label] quality section exists (overall={overall})")
else:
    print(f"  FAIL: [$label] quality section missing or invalid")
    errors.append(1)

# limitations section exists
lim = d.get('limitations', [])
if isinstance(lim, list) and len(lim) > 0:
    print(f"  PASS: [$label] limitations section exists ({len(lim)} items)")
else:
    print(f"  FAIL: [$label] limitations section missing or empty")
    errors.append(1)

sys.exit(len(errors))
PYEOF
  local rc=$?
  return $rc
}

validate_snapshot "Rust" "$RUST_OUT"
validate_snapshot "TypeScript" "$TS_OUT"

# ════════════════════════════════════════════════════════════════════
section "Summary"
# ════════════════════════════════════════════════════════════════════

TOTAL=$((PASS + FAIL))
echo ""
echo -e "${BOLD}Results:${RESET}"
echo -e "  ${GREEN}${PASS} passed${RESET}, ${RED}${FAIL} failed${RESET}, ${TOTAL} total"

if [[ $FAIL -gt 0 ]]; then
  echo ""
  echo -e "${RED}${BOLD}SMOKE FAILED${RESET}"
  exit 1
else
  echo ""
  echo -e "${GREEN}${BOLD}SMOKE PASSED${RESET}"

  if [[ "$KEEP_TEMP" == true ]]; then
    echo "Temp files preserved in: ${TEMP_DIR}"
    echo "  Rust:     ${RUST_OUT}"
    echo "  TypeScript: ${TS_OUT}"
  fi
  exit 0
fi
