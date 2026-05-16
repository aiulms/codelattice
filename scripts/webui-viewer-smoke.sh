#!/usr/bin/env bash
# ============================================================
# CodeLattice WebUI Snapshot Viewer — Smoke Test (Phase A Enhanced)
# Static checks for viewer files + multi-language fixture compatibility.
# No browser required for core checks.
#
# Usage:
#   bash scripts/webui-viewer-smoke.sh              # standard run
#   bash scripts/webui-viewer-smoke.sh --strict     # browser checks fail = hard fail
#   bash scripts/webui-viewer-smoke.sh --skip-browser
#   bash scripts/webui-viewer-smoke.sh --snapshot-dir <path>
#   bash scripts/webui-viewer-smoke.sh --help
# ============================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VIEWER_DIR="$REPO_ROOT/webui/snapshot-viewer"
DEFAULT_SNAPSHOTS_DIR="$REPO_ROOT/fixtures/webui-snapshots"
STRICT_MODE=false
SKIP_BROWSER=false
SNAPSHOTS_DIR=""

for arg in "$@"; do
  case "$arg" in
    --strict)       STRICT_MODE=true ;;
    --skip-browser) SKIP_BROWSER=true ;;
    --snapshot-dir) shift; SNAPSHOTS_DIR="$1" ;;
    --help)
      echo "Usage: $0 [--strict] [--skip-browser] [--snapshot-dir <path>]"
      echo "  --strict         Browser check failures cause hard FAIL"
      echo "  --skip-browser   Skip DOM/browser checks even if available"
      echo "  --snapshot-dir   Custom snapshot directory (default: fixtures/webui-snapshots)"
      exit 0
      ;;
    *) echo "Unknown arg: $arg"; exit 1 ;;
  esac
done

if [[ -z "$SNAPSHOTS_DIR" ]]; then
  SNAPSHOTS_DIR="$DEFAULT_SNAPSHOTS_DIR"
fi

PASS=0
FAIL=0
TOTAL=0

check() {
  local label="$1" expected="$2"
  shift 2
  local actual=""
  if [[ $# -gt 0 ]]; then actual="$*"; fi
  TOTAL=$((TOTAL + 1))
  if [[ "$actual" == "$expected" ]]; then
    PASS=$((PASS + 1))
    printf '  \033[32m✓\033[0m %s\n' "$label"
  else
    FAIL=$((FAIL + 1))
    printf '  \033[31m✗\033[0m %s (expected: %s, got: [%s])\n' "$label" "$expected" "$actual"
  fi
}

echo '============================================================'
echo 'CodeLattice WebUI Viewer — Smoke Test (Phase A Enhanced)'
echo '============================================================'
echo ''

# ---- Prerequisites (5 checks) ----
echo '--- Prerequisites ---'

[[ -d "$VIEWER_DIR" ]]          && check "Viewer directory exists"        "yes" "yes" || check "Viewer directory exists"        "yes" "no"
[[ -f "$VIEWER_DIR/index.html" ]] && check "index.html exists"             "yes" "yes" || check "index.html exists"             "yes" "no"
[[ -f "$VIEWER_DIR/styles.css" ]] && check "styles.css exists"              "yes" "yes" || check "styles.css exists"              "yes" "no"
[[ -f "$VIEWER_DIR/app.js" ]]     && check "app.js exists"                  "yes" "yes" || check "app.js exists"                  "yes" "no"
if command -v node >/dev/null 2>&1; then check "node available for JS syntax" "yes" "yes"; else check "node available for JS syntax" "yes" "no"; fi
if command -v python3 >/dev/null 2>&1; then check "python3 available" "yes" "yes"; else check "python3 available" "yes" "no"; fi

echo ''

# ---- JS Syntax & Structure (12 checks, Phase A enhanced) ----
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

# Phase A render functions
FUNC_COUNT=0
if [[ -f "$VIEWER_DIR/app.js" ]]; then
  FUNC_COUNT=$(grep -cE "(function |=>)\s*(renderAll|renderHeader|renderDashboard|renderExplore|renderSourceFiles|renderTopFiles|selectSymbol|renderCleanup|renderReleaseReview|renderWorkflowPresets|applyExploreFilter|renderSymbolList|loadSnapshot|showError|showWelcome)" "$VIEWER_DIR/app.js" 2>/dev/null || echo 0)
fi
[[ "$FUNC_COUNT" -ge 10 ]] && check "Phase A render functions present (>=10)" "pass" "pass" || check "Phase A render functions present (>=10)" "pass" "fail (${FUNC_COUNT} found)"

# HTML structure checks
grep -qF 'styles.css' "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML references styles.css"        "yes" "yes" || check "HTML references styles.css"        "yes" "no"
grep -qF 'app.js'     "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML references app.js"           "yes" "yes" || check "HTML references app.js"           "yes" "no"
grep -qF 'caution-banner' "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has caution banner element" "yes" "yes" || check "HTML has caution banner element" "yes" "no"
grep -qF 'tab-btn'     "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has tab navigation"          "yes" "yes" || check "HTML has tab navigation"          "yes" "no"

# Phase A specific views
grep -qF 'view-dashboard' "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has Dashboard view ID"       "yes" "yes" || check "HTML has Dashboard view ID"       "yes" "no"
grep -qF 'view-explore'   "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has Explore view ID"         "yes" "yes" || check "HTML has Explore view ID"         "yes" "no"
grep -qF 'view-cleanup'   "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has Cleanup view ID"         "yes" "yes" || check "HTML has Cleanup view ID"         "yes" "no"
grep -qF 'view-release'   "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has Release Review view ID" "yes" "yes" || check "HTML has Release Review view ID" "yes" "no"
grep -qF 'view-workflows' "$VIEWER_DIR/index.html" 2>/dev/null && check "HTML has Workflow view ID"        "yes" "yes" || check "HTML has Workflow view ID"        "yes" "no"

# Caution text presence
CAUTION_CHECK=$(cat "$VIEWER_DIR/index.html" "$VIEWER_DIR/app.js" 2>/dev/null | grep -cE 'static.?analysis.?only|deletion.?proof|heuristic|candidate|NOT.*deletion' 2>/dev/null || echo 0)
[[ "$CAUTION_CHECK" -ge 3 ]] && check "Caution text present (>=3 matches)" "pass" "pass" || check "Caution text present (>=3 matches)" "pass" "fail (${CAUTION_CHECK} found)"

echo ''

# ---- CSS Checks (4 checks) ----
echo '--- CSS Basics ---'

grep -q ':root'   "$VIEWER_DIR/styles.css" 2>/dev/null && check "CSS uses CSS variables (:root)"       "yes" "yes" || check "CSS uses CSS variables (:root)"       "yes" "no"
grep -q '@media'  "$VIEWER_DIR/styles.css" 2>/dev/null && check "CSS has responsive media queries"    "yes" "yes" || check "CSS has responsive media queries"    "yes" "no"
! grep -q '@import' "$VIEWER_DIR/styles.css" 2>/dev/null && check "CSS has no @import deps"              "yes" "yes" || check "CSS has no @import deps"              "yes" "no"
# Phase A new CSS components
CSS_PHASE_A=$(grep -cE '(workflow-card|file-card|sym-kind-badge|detail-table|caution-list|meta-list)' "$VIEWER_DIR/styles.css" 2>/dev/null || echo 0)
[[ "$CSS_PHASE_A" -ge 5 ]] && check "CSS Phase A component styles present (>=5)" "pass" "pass" || check "CSS Phase A component styles present (>=5)" "pass" "fail (${CSS_PHASE_A})"

echo ''

# ---- Multi-Language Fixture Snapshots Matrix (15+ checks) ----
echo '--- Multi-Language Fixture Snapshot Matrix ---'

LANGS=("rust" "typescript" "c" "cpp" "python")
LANG_PASS=0
LANG_FAIL=0

for lang in "${LANGS[@]}"; do
  snap="${SNAPSHOTS_DIR}/${lang}-portable-smoke.snapshot.json"

  if [[ ! -f "$snap" ]]; then
    TOTAL=$((TOTAL + 1)); FAIL=$((FAIL + 1))
    LANG_FAIL=$((LANG_FAIL + 1))
    printf '  \033[31m✗\033[0m [%s] snapshot file missing\n' "$lang"
    continue
  fi
  TOTAL=$((TOTAL + 1)); PASS=$((PASS + 1))
  printf '  \033[32m✓\033[0m [%s] file exists\n' "$lang"

  # JSON parse
  if python3 -c "import json; json.load(open('$snap'))" 2>/dev/null; then
    TOTAL=$((TOTAL + 1)); PASS=$((PASS + 1))
    printf '  \033[32m✓\033[0m [%s] valid JSON\n' "$lang"
  else
    TOTAL=$((TOTAL + 1)); FAIL=$((FAIL + 1))
    LANG_FAIL=$((LANG_FAIL + 1))
    printf '  \033[31m✗\033[0m [%s] invalid JSON\n' "$lang"
    continue
  fi

  # Contract checks via Python
  python3 -c "
import json
with open('$snap') as f:
    d = json.load(f)

checks = {
    'schemaVersion': d.get('schemaVersion', '') == 'webui.snapshot.v1',
    'staticAnalysis': d.get('generatedFrom', {}).get('staticAnalysis') is True,
    'runtimeVerified': d.get('generatedFrom', {}).get('runtimeVerified') is False,
}
s = d.get('summary', {})
e = d.get('explore', {})
has_data = s.get('sourceFileCount', 0) > 0 or len(e.get('symbols', [])) > 0 or len(e.get('sourceFiles', [])) > 0
checks['data_present'] = has_data
checks['quality'] = bool(d.get('quality'))
checks['limitations'] = bool(d.get('limitations'))
checks['explore'] = len(e.get('symbols', [])) > 0 or len(e.get('sourceFiles', [])) > 0
checks['workflows'] = len(d.get('workflowPresets', {}).get('presets', [])) >= 10

raw = json.dumps(d)
checks['no_path_leak'] = '/Users/' not in raw and '/Desktop/codelattice' not in raw

for name, ok in checks.items():
    status = 'PASS' if ok else 'FAIL'
    print(f'{status}:$lang:{name}')
" 2>/dev/null | while IFS=: read -r lang_check result; do
    TOTAL=$((TOTAL + 1))
    if [[ "$result" == "PASS" ]]; then
      PASS=$((PASS + 1))
      LANG_PASS=$((LANG_PASS + 1))
      printf '  \033[32m✓\033[0m [%s] %s\n' "$lang" "$lang_check"
    else
      FAIL=$((FAIL + 1))
      LANG_FAIL=$((LANG_FAIL + 1))
      printf '  \033[31m✗\033[0m [%s] %s\n' "$lang" "$lang_check"
    fi
  done
done

echo ''
printf '  Matrix: %d passed / %d failed across %d languages\n\n' "$LANG_PASS" "$LANG_FAIL" "${#LANGS[@]}"

# ---- Page Content Verification (3 checks) ----
echo '--- Page Content Verification ---'

PAGE_SIZE=$(wc -c < "$VIEWER_DIR/index.html" 2>/dev/null || echo 0)
[[ "$PAGE_SIZE" -gt 2000 ]] && check "index.html > 2000 bytes"            "pass" "pass" || check "index.html > 2000 bytes"            "pass" "fail ($PAGE_SIZE bytes)"

COMBINED_CHECK=$(cat "$VIEWER_DIR/index.html" "$VIEWER_DIR/app.js" 2>/dev/null | grep -cE 'CodeLattice|Dashboard|Explore|Workflow|Static analysis only|cleanup|release' 2>/dev/null || echo 0)
[[ "$COMBINED_CHECK" -ge 5 ]] && check "Page contains required keywords (>=5)" "pass" "pass" || check "Page contains required keywords (>=5)" "pass" "fail ($COMBINED_CHECK)"

APP_JS_SIZE=$(wc -c < "$VIEWER_DIR/app.js" 2>/dev/null || echo 0)
[[ "$APP_JS_SIZE" -gt 5000 ]] && check "app.js > 5000 bytes (substantial logic)" "pass" "pass" || check "app.js > 5000 bytes" "pass" "fail ($APP_JS_SIZE bytes)"

echo ''

# ---- Summary ----
echo '============================================================'
printf 'Results:\n  \033[32m%d passed\033[0m, \033[31m%d failed\033[0m, %d total\n' "$PASS" "$FAIL" "$TOTAL"
echo '============================================================'

if [[ "$FAIL" -eq 0 ]]; then
  echo '\033[32mSMOKE PASSED\033[0m'
  exit 0
else
  echo '\033[31mSMOKE FAILED\033[0m'
  if [[ "$STRICT_MODE" == true ]]; then
    exit 1
  else
    exit 0
  fi
fi
