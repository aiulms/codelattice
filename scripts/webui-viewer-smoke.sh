#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
VD="$WS/webui/snapshot-viewer"
SN="$WS/fixtures/webui-snapshots"
ST=false; SB=false
for a in "$@"; do
  case "$a" in --strict) ST=true ;; --skip-browser) SB=true ;; --snapshot-dir) shift; SN="$1" ;; --help) echo "Usage: $0 [--strict] [--skip-browser] [--snapshot-dir <path>]"; exit 0 ;; *) echo "Unknown: $a"; exit 1 ;; esac
done
P=0; FA=0; T=0
chk() { local l="$1" e="$2" a="${3:-}"; T=$((T+1)); if [[ "$a" == "$e" ]]; then P=$((P+1)); printf '  \033[32m✓\033[0m %s\n' "$l"; else FA=$((FA+1)); printf '  \033[31m✗\033[0m %s (got:%s)\n' "$l" "$a"; fi; }
echo "CodeLattice Viewer Smoke (Phase B)"
echo ""; echo "--- Prerequisites ---"
[[ -d "$VD" ]] && chk "viewer dir" yes yes || chk "viewer dir" yes no
[[ -f "$VD/index.html" ]] && chk "index.html" yes yes || chk "index.html" yes no
[[ -f "$VD/styles.css" ]] && chk "styles.css" yes yes || chk "styles.css" yes no
[[ -f "$VD/app.js" ]] && chk "app.js" yes yes || chk "app.js" yes no
[[ -f "$VD/timeline.js" ]] && chk "timeline.js" yes yes || chk "timeline.js" yes no
[[ -f "$VD/report.js" ]] && chk "report.js" yes yes || chk "report.js" yes no
HAS_NODE=no; command -v node >/dev/null 2>&1 && HAS_NODE=yes; chk "node" yes "$HAS_NODE"
HAS_PY=no; command -v python3 >/dev/null 2>&1 && HAS_PY=yes; chk "python3" yes "$HAS_PY"
echo ""; echo "--- JS Syntax ---"
if [[ "$HAS_NODE" == yes ]]; then
  for f in app.js timeline.js report.js runner.js; do
    node -c "$VD/$f" >/dev/null 2>&1 && chk "$f syntax" ok ok || chk "$f syntax" ok fail
  done
fi
# Phase B functions
FC=$(grep -cE '(function |=>)\s*(renderAll|renderHeader|renderDashboard|renderExplore|renderSourceFiles|selectSymbol|renderCleanup|renderReleaseReview|renderWorkflowPresets|renderGraph|renderDiff|computeDiff|loadSnapshot|showError|showWelcome)' "$VD/app.js" 2>/dev/null || echo 0)
[[ $FC -ge 12 ]] && chk "core functions (>=12)" pass pass || chk "core functions (>=12)" pass "fail($FC)"
echo ""; echo "--- HTML Structure ---"
grep -qF 'styles.css' "$VD/index.html" && chk "css ref" yes yes || chk "css ref" yes no
grep -qF 'app.js' "$VD/index.html" && chk "js ref" yes yes || chk "js ref" yes no
grep -qF 'caution-banner' "$VD/index.html" && chk "caution banner" yes yes || chk "caution banner" yes no
grep -qF 'tab-btn' "$VD/index.html" && chk "tab nav" yes yes || chk "tab nav" yes no
for v in dashboard explore graph cleanup release workflows diff; do
  grep -qF "view-$v" "$VD/index.html" && chk "view:$v" yes yes || chk "view:$v" yes no
done
CAUT=$(cat "$VD/index.html" "$VD/app.js" 2>/dev/null | grep -cE 'static.?analysis.?only|deletion.?proof|heuristic|candidate|NOT.*deletion' 2>/dev/null || echo 0)
[[ $CAUT -ge 4 ]] && chk "caution text (>=4)" pass pass || chk "caution text (>=4)" pass "fail($CAUT)"
echo ""; echo "--- CSS ---"
grep -q ':root' "$VD/styles.css" && chk "css vars" yes yes || chk "css vars" yes no
grep -q '@media' "$VD/styles.css" && chk "responsive" yes yes || chk "responsive" yes no
! grep -q '@import' "$VD/styles.css" && chk "no @import" yes yes || chk "no @import" yes no
echo ""; echo "--- Fixture Matrix ---"
REQ=(rust typescript c cpp python)
TD=$(mktemp -d)
trap "rm -rf $TD" EXIT
MP=0; MF=0
for L in "${REQ[@]}"; do
  F="$SN/${L}-portable-smoke.snapshot.json"
  if [[ ! -f "$F" ]]; then
    MF=$((MF+1)); printf '  \033[31m✗\033[0m [%s] file missing\n' "$L"; continue
  fi
  if ! python3 -c "import json;json.load(open('$F'))" 2>/dev/null; then
    MF=$((MF+1)); printf '  \033[31m✗\033[0m [%s] invalid JSON\n' "$L"; continue
  fi
  python3 -c "
import json
with open('$F') as fh: d=json.load(fh)
ok=True
sd=d.get('summary',{}); e=d.get('explore',{})
# Phase B: allow explore.symbols as data even when summary.sourceFileCount==0
has_data = sd.get('sourceFileCount',0)>0 or len(e.get('symbols',[]))>0 or len(e.get('sourceFiles',[]))>0
if not has_data: ok=False
if not d.get('quality'): ok=False
if not d.get('limitations'): ok=False
raw=json.dumps(d)
if '/Users/' in raw or 'Desktop/codelattice' in raw: ok=False
print('ok' if ok else 'fail')
" > "$TD/${L}_check.txt" 2>/dev/null
  R=$(cat "$TD/${L}_check.txt" 2>/dev/null || echo fail)
  if [[ "$R" == ok ]]; then MP=$((MP+1)); printf '  \033[32m✓\033[0m [%s]\n' "$L"; else MF=$((MF+1)); printf '  \033[31m✗\033[0m [%s]\n' "$L"; fi
done
MT=$((MP+MF))
printf '  Matrix: %d/%d pass\n' "$MP" "$MT"
[[ $MF -gt 0 ]] && chk "matrix all pass" pass "fail($MF failed)"
echo ""; echo "--- Phase D Runner Checks ---"
[[ -f "$VD/runner.js" ]] && chk "runner.js exists" yes yes || chk "runner.js exists" yes no
for f in runner.js; do node -c "$VD/$f" >/dev/null 2>&1 && chk "$f syntax" ok ok || chk "$f syntax" ok fail; done
RD_FC=$(grep -cE '(runnerCheckHealth|runnerGenerate|runnerLoadLibrary|runnerLoadSnapshot|runnerCompareSnapshot|runnerAddTimeline|renderSnapshotLibrary|runnerApi)' "$VD/runner.js" 2>/dev/null || echo 0)
[[ $RD_FC -ge 6 ]] && chk "runner functions (>=6)" pass pass || chk "runner functions (>=6)" pass "fail($RD_FC)"
grep -qF "runner-panel" "$VD/index.html" && chk "runner panel html" yes yes || chk "runner panel html" yes no
grep -qF "runner-mode-badge" "$VD/index.html" && chk "runner badge" yes yes || chk "runner badge" yes no

echo ""; echo "--- Phase C JS Syntax (timeline.js + report.js) ---"
for f in timeline.js report.js; do
  [[ -f "$VD/$f" ]] && chk "$f exists" yes yes || chk "$f exists" yes no
  node -c "$VD/$f" >/dev/null 2>&1 && chk "$f syntax" ok ok || chk "$f syntax" ok fail
done
# Phase C core functions (across all JS files)
PC_FC=$(grep -cE '(loadTimeline|buildTimelineData|renderTimeline|renderTimelineChart|timelineMetricValue|selectTimelineMetric|generateMarkdownReport|collectReportContext|renderReport|copyReport|downloadReport|buildWorkflowChecklist|renderWorkflowChecklist|toggleChecklistItem|resetWorkflowChecklist)' "$VD/timeline.js" "$VD/report.js" 2>/dev/null | awk -F: '{s+=$NF}END{print s+0}' || echo 0)
[[ $PC_FC -ge 12 ]] && chk "Phase C functions (>=12)" pass pass || chk "Phase C functions (>=12)" pass "fail($PC_FC)"
# Phase C UI elements
for v in timeline report; do grep -qF "view-$v" "$VD/index.html" && chk "view:$v" yes yes || chk "view:$v" yes no; done
# Workflow checklist upgrade
grep -qF "toggleChecklistItem" "$VD/report.js" && chk "checklist toggle" yes yes || chk "checklist toggle" yes no
grep -qF "resetWorkflowChecklist" "$VD/report.js" && chk "checklist reset" yes yes || chk "checklist reset" yes no
# Report export
grep -qF "generateMarkdownReport" "$VD/report.js" && chk "markdown report" yes yes || chk "markdown report" yes no
PSZ=$(wc -c < "$VD/index.html" 2>/dev/null || echo 0); [[ $PSZ -gt 3000 ]] && chk "index.html >3KB" pass pass || chk "index.html >3KB" pass "fail($PSZ)"
KW=$(cat "$VD/index.html" "$VD/app.js" 2>/dev/null | grep -cE 'CodeLattice|Dashboard|Explore|Graph|Workflow|Diff|Static analysis only|cleanup|release' 2>/dev/null || echo 0)
[[ $KW -ge 6 ]] && chk "keywords (>=6)" pass pass || chk "keywords (>=6)" pass "fail($KW)"
TOJS=$(cat "$VD/app.js" "$VD/timeline.js" "$VD/report.js" 2>/dev/null | wc -c | tr -d ' ')
[[ $TOJS -gt 15000 ]] && chk "total JS >15KB" pass pass || chk "total JS >15KB" pass "fail($TOJS)"
echo ""; echo "========================================"
printf "Results: \033[32m%d passed\033[0m, \033[31m%d failed\033[0m, %d total\n" "$P" "$FA" "$T"
if [[ $FA -gt 0 ]]; then echo "SMOKE FAILED"; exit 1; fi
echo "SMOKE PASSED"
