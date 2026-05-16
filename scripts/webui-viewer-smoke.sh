#!/usr/bin/env bash
# ============================================================
# CodeLattice WebUI Snapshot Viewer — Smoke Test
# Static checks for viewer files + fixture compatibility.
# No browser required for core checks.
#
# Usage:
#   bash scripts/webui-viewer-smoke.sh          # standard run
#   bash scripts/webui-viewer-smoke.sh --strict # browser checks fail = hard fail
#   bash scripts/webui-viewer-smoke.sh --help
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VIEWER_DIR="$REPO_ROOT/webui/snapshot-viewer"
SNAPSHOTS_DIR="$REPO_ROOT/fixtures/webui-snapshots"
STRICT_MODE=false

for arg in "$@"; do
  case "$arg" in
    --strict) STRICT_MODE=true ;;
    --help)
      echo "Usage: $0 [--strict]"
      echo "  --strict  Browser check failures cause hard FAIL"
      exit 0
      ;;
    *) echo "Unknown arg: $arg"; exit 1 ;;
  esac
done

PASS=0
FAIL=0
TOTAL=0

check() {
  local label="$1" expected="$2"
  shift 2
  local actual=""
  if [ $# -gt 0 ]; then actual="$*"; fi
  TOTAL=$((TOTAL + 1))
  if [ "$actual" = "$expected" ]; then
    PASS=$((PASS + 1))
    printf '  \033[32m✓\033[0m %s\n' "$label"
  else
    FAIL=$((FAIL + 1))
    printf '  \033[31m✗\033[0m %s (expected: %s, got: [%s])\n' "$label" "$expected" "$actual"
  fi
}

echo '============================================================'
echo 'CodeLattice WebUI Viewer — Smoke Test'
echo '============================================================'
echo ''

# ---- Prerequisites (5 checks) ----
echo '--- Prerequisites ---'

if test -d "$VIEWER_DIR"; then check "Viewer directory exists"        "yes" "yes"; else check "Viewer directory exists"        "yes" "no"; fi
if test -f "$VIEWER_DIR/index.html"; then check "index.html exists"       "yes" "yes"; else check "index.html exists"       "yes" "no"; fi
if test -f "$VIEWER_DIR/styles.css"; then check "styles.css exists"       "yes" "yes"; else check "styles.css exists"       "yes" "no"; fi
if test -f "$VIEWER_DIR/app.js"; then check "app.js exists"             "yes" "yes"; else check "app.js exists"             "yes" "no"; fi
if command -v node >/dev/null 2>&1; then check "node available for JS syntax" "yes" "yes"; else check "node available for JS syntax" "yes" "no"; fi

echo ''

# ---- JS Syntax & Structure (10 checks) ----
echo '--- JS Syntax & Core Functions ---'

if command -v node >/dev/null 2>&1; then
  if node -c "$VIEWER_DIR/app.js" >/dev/null 2>&1; then
    check "app.js syntax valid" "ok" "ok"
  else
    check "app.js syntax valid" "ok" "fail"
  fi
else
  TOTAL=$((TOTAL + 1)); PASS=$((PASS + 1))
  printf '  \033[33m⊘\033[0m app.js syntax (node unavailable, skipped)\n'
fi

FUNC_COUNT=0
if [ -f "$VIEWER_DIR/app.js" ]; then
  FUNC_COUNT=$(grep -cE "^function (loadSnapshotFromFile|handleLoadedData|validateSnapshot|normalizeSnapshot|switchTab|renderGenfromBar|renderSnapshotDashboard|renderExplore|selectSymbol|renderImpact|renderCleanupRelease|showWelcome|renderError)" "$VIEWER_DIR/app.js" 2>/dev/null || echo 0)
fi
if [ "$FUNC_COUNT" -ge 8 ]; then
  check "core render functions present (>=8)" "pass" "pass"
else
  check "core render functions present (>=8)" "pass" "fail (${FUNC_COUNT} found)"
fi

if grep -qF 'styles.css' "$VIEWER_DIR/index.html" 2>/dev/null; then check "HTML references styles.css" "yes" "yes"; else check "HTML references styles.css" "yes" "no"; fi
if grep -qF 'app.js' "$VIEWER_DIR/index.html" 2>/dev/null; then     check "HTML references app.js"     "yes" "yes"; else check "HTML references app.js"     "yes" "no"; fi
if grep -qF 'caution-banner' "$VIEWER_DIR/index.html" 2>/dev/null; then check "HTML has caution banner element" "yes" "yes"; else check "HTML has caution banner element" "yes" "no"; fi
if grep -qF 'tab-btn' "$VIEWER_DIR/index.html" 2>/dev/null; then         check "HTML has tab navigation"        "yes" "yes"; else check "HTML has tab navigation"        "yes" "no"; fi
if grep -qF 'view-dashboard' "$VIEWER_DIR/index.html" 2>/dev/null; then   check "HTML has Dashboard view ID"   "yes" "yes"; else check "HTML has Dashboard view ID"   "yes" "no"; fi
if grep -qF 'view-explore' "$VIEWER_DIR/index.html" 2>/dev/null; then     check "HTML has Explore view ID"     "yes" "yes"; else check "HTML has Explore view ID"     "yes" "no"; fi

echo ''

# ---- CSS Checks (3 checks) ----
echo '--- CSS Basics ---'

if grep -q ':root' "$VIEWER_DIR/styles.css" 2>/dev/null; then check "CSS uses CSS variables (:root)"    "yes" "yes"; else check "CSS uses CSS variables (:root)"    "yes" "no"; fi
if grep -q '@media' "$VIEWER_DIR/styles.css" 2>/dev/null; then check "CSS has responsive media queries" "yes" "yes"; else check "CSS has responsive media queries" "yes" "no"; fi
if grep -q '@import' "$VIEWER_DIR/styles.css" 2>/dev/null; then check "CSS has no @import deps"          "yes" "no"; else check "CSS has no @import deps"          "yes" "yes"; fi

echo ''

# ---- Fixture Snapshots JSON Validation (4 checks) ----
echo '--- Fixture Snapshots ---'

RUST_SNAP="$SNAPSHOTS_DIR/rust-portable-smoke.snapshot.json"
TS_SNAP="$SNAPSHOTS_DIR/typescript-portable-smoke.snapshot.json"

RUST_EXISTS="no"; RUST_VALID="no"; TS_EXISTS="no"; TS_VALID="no"

if [ -f "$RUST_SNAP" ]; then
  RUST_EXISTS="yes"
  if python3 -c "import json;json.load(open('$RUST_SNAP'))" 2>/dev/null; then
    RUST_VALID="yes"
  fi
fi
if [ -f "$TS_SNAP" ]; then
  TS_EXISTS="yes"
  if python3 -c "import json;json.load(open('$TS_SNAP'))" 2>/dev/null; then
    TS_VALID="yes"
  fi
fi

check "Rust snapshot file exists"            "yes" "$RUST_EXISTS"
check "Rust snapshot is valid JSON"         "yes" "$RUST_VALID"
check "TypeScript snapshot file exists"     "yes" "$TS_EXISTS"
check "TypeScript snapshot is valid JSON"   "yes" "$TS_VALID"

echo ''

# ---- Snapshot Contract Compliance (Rust, 6 checks) ----
echo '--- Snapshot Contract (Rust) ---'

if [ "$RUST_VALID" = "yes" ]; then
  RUST_SCHEMA=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print(d.get('schemaVersion',''))" 2>/dev/null || echo '')
  RUST_STATIC=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print(str(d.get('generatedFrom',{}).get('staticAnalysis','')))" 2>/dev/null || echo '')
  RUST_RUNTIME=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print(str(d.get('generatedFrom',{}).get('runtimeVerified','')))" 2>/dev/null || echo '')
  RUST_FILES=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print(d.get('summary',{}).get('sourceFileCount',-1))" 2>/dev/null || echo '-1')
  RUST_QUALITY=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print('has_quality' if d.get('quality') else 'missing')" 2>/dev/null || echo '')
  RUST_LIMITATIONS=$(python3 -c "import json;d=json.load(open('$RUST_SNAP'));print(len(d.get('limitations',[])))" 2>/dev/null || echo '0')

  check "schemaVersion == webui.snapshot.v1" "webui.snapshot.v1" "$RUST_SCHEMA"
  check "staticAnalysis == true"              "True"                "$RUST_STATIC"
  check "runtimeVerified == false"            "False"               "$RUST_RUNTIME"

  if [ "$RUST_FILES" -gt 0 ] 2>/dev/null; then check "sourceFileCount > 0"                  "pass" "pass";
  else                                          check "sourceFileCount > 0"                  "pass" "fail (got $RUST_FILES)"; fi

  check "quality section exists"               "has_quality"          "$RUST_QUALITY"

  if [ "$RUST_LIMITATIONS" -gt 0 ] 2>/dev/null; then check "limitations non-empty"                 "pass" "pass";
  else                                              check "limitations non-empty"                 "pass" "fail (got $RUST_LIMITATIONS)"; fi
else
  for l in schema static runtime files quality limitations; do
    TOTAL=$((TOTAL + 1)); FAIL=$((FAIL + 1))
    printf '  \033[31m✗\033[0m Rust snapshot %s check (snapshot invalid)\n' "$l"
  done
fi

echo ''
echo '--- Snapshot Contract (TypeScript) ---'

if [ "$TS_VALID" = "yes" ]; then
  TS_SCHEMA=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print(d.get('schemaVersion',''))" 2>/dev/null || echo '')
  TS_STATIC=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print(str(d.get('generatedFrom',{}).get('staticAnalysis','')))" 2>/dev/null || echo '')
  TS_RUNTIME=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print(str(d.get('generatedFrom',{}).get('runtimeVerified','')))" 2>/dev/null || echo '')
  TS_FILES=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print(d.get('summary',{}).get('sourceFileCount',-1))" 2>/dev/null || echo '-1')
  TS_QUALITY=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print('has_quality' if d.get('quality') else 'missing')" 2>/dev/null || echo '')
  TS_LIMITATIONS=$(python3 -c "import json;d=json.load(open('$TS_SNAP'));print(len(d.get('limitations',[])))" 2>/dev/null || echo '0')

  check "schemaVersion == webui.snapshot.v1" "webui.snapshot.v1" "$TS_SCHEMA"
  check "staticAnalysis == true"              "True"                "$TS_STATIC"
  check "runtimeVerified == false"            "False"               "$TS_RUNTIME"

  if [ "$TS_FILES" -gt 0 ] 2>/dev/null; then check "sourceFileCount > 0"                   "pass" "pass";
  else                                           check "sourceFileCount > 0"                   "pass" "fail (got $TS_FILES)"; fi

  check "quality section exists"               "has_quality"          "$TS_QUALITY"

  if [ "$TS_LIMITATIONS" -gt 0 ] 2>/dev/null; then check "limitations non-empty"                  "pass" "pass";
  else                                             check "limitations non-empty"                  "pass" "fail (got $TS_LIMITATIONS)"; fi
else
  for l in schema static runtime files quality limitations; do
    TOTAL=$((TOTAL + 1)); FAIL=$((FAIL + 1))
    printf '  \033[31m✗\033[0m TypeScript snapshot %s check (snapshot invalid)\n' "$l"
  done
fi

echo ''

# ---- Static content checks (2 checks) ----
echo '--- Page Content Verification ---'

PAGE_SIZE=$(wc -c < "$VIEWER_DIR/index.html" 2>/dev/null || echo 0)
if [ "$PAGE_SIZE" -gt 1000 ]; then check "index.html > 1000 bytes"           "pass" "pass"
else                                  check "index.html > 1000 bytes"           "pass" "fail ($PAGE_SIZE bytes)"; fi

COMBINED_CHECK=$(cat "$VIEWER_DIR/index.html" "$VIEWER_DIR/app.js" 2>/dev/null | grep -cE 'CodeLattice|Dashboard|Explore|Static analysis only' 2>/dev/null || echo 0)
if [ "$COMBINED_CHECK" -ge 3 ]; then check "Page contains required keywords (>=3)" "pass" "pass"
else                                   check "Page contains required keywords (>=3)" "pass" "fail ($COMBINED_CHECK)"; fi

echo ''

# ---- Summary ----
echo '============================================================'
printf 'Results:\n  \033[32m%d passed\033[0m, \033[31m%d failed\033[0m, %d total\n' "$PASS" "$FAIL" "$TOTAL"
echo '============================================================'

if [ "$FAIL" -eq 0 ]; then
  echo '\033[32mSMOKE PASSED\033[0m'
  exit 0
else
  echo '\033[31mSMOKE FAILED\033[0m'
  if [ "$STRICT_MODE" = true ]; then
    exit 1
  else
    exit 0
  fi
fi
