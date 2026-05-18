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
[[ -f "$VD/graph-g6.js" ]] && chk "graph-g6.js" yes yes || chk "graph-g6.js" yes no
[[ -f "$VD/vendor/g6/g6.min.js" ]] && chk "vendored G6" yes yes || chk "vendored G6" yes no
HAS_NODE=no; command -v node >/dev/null 2>&1 && HAS_NODE=yes; chk "node" yes "$HAS_NODE"
HAS_PY=no; command -v python3 >/dev/null 2>&1 && HAS_PY=yes; chk "python3" yes "$HAS_PY"
echo ""; echo "--- JS Syntax ---"
if [[ "$HAS_NODE" == yes ]]; then
  for f in app.js timeline.js report.js runner.js live.js graph-g6.js vendor/g6/g6.min.js; do
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
CAUT=$(cat "$VD/index.html" "$VD/app.js" 2>/dev/null | grep -cE 'static.?analysis.?only|deletion.?proof|heuristic|candidate|NOT.*deletion|静态分析|静态审查|删除|启发式|候选' 2>/dev/null || echo 0)
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
echo ""; echo "--- Phase G Live MCP Checks ---"
[[ -f "$VD/live.js" ]] && chk "live.js exists" yes yes || chk "live.js exists" yes no
for f in live.js; do node -c "$VD/$f" >/dev/null 2>&1 && chk "$f syntax" ok ok || chk "$f syntax" ok fail; done
grep -qF "live-mcp-panel" "$VD/index.html" && chk "live panel html" yes yes || chk "live panel html" yes no
LG_FC=$(grep -cE '(liveCheckMcp|liveLoadTools|liveCreateJob|livePollJobs|renderLiveJobs|renderLiveStatus|liveCancelJob|liveDeleteJob|liveViewResult)' "$VD/live.js" 2>/dev/null||echo 0)
[[ $LG_FC -ge 6 ]] && chk "live functions (>=6)" pass pass || chk "live functions (>=6)" pass "fail($LG_FC)"

echo ""; echo "--- Phase E Workbench Checks ---"
# Profiles
grep -qF "runner-profiles-list" "$VD/index.html" && chk "profiles html" yes yes || chk "profiles html" yes no
grep -qF "runnerGenForProfile" "$VD/runner.js" && chk "profile_gen" yes yes || chk "profile gen" yes no
grep -qF "createProfile" "$VD/runner.js" && chk "create_profile" yes yes || chk "create_profile" yes no
# Guided Review
grep -qF "guided-review-panel" "$VD/index.html" && chk "guided html" yes yes || chk "guided html" yes no
grep -qF "GUIDED_SCENARIOS" "$VD/runner.js" && chk "scenarios" yes yes || chk "scenarios" yes no
grep -qF "guidedSelect" "$VD/runner.js" && chk "guided_select" yes yes || chk "guided_select" yes no
grep -qF "guidedRender" "$VD/runner.js" && chk "guided_render" yes yes || chk "guided_render" yes no
# Report templates
grep -qF "getReportTemplates" "$VD/report.js" && chk "templates" yes yes || chk "templates" yes no
grep -qF "report-template-select" "$VD/report.js" && chk "template_sel" yes yes || chk "template_sel" yes no
# Phase E JS functions count
PE_FC=$(grep -cE '(loadProfiles|createProfile|updateProfile|deleteProfile|runnerGenForProfile|renderSnapshotLibrary|guidedRender|guidedSelect|guidedToggle|getReportTemplates|generateTemplateReport|selectReportTemplate|rebuildIndex)' "$VD/runner.js" "$VD/report.js" 2>/dev/null|awk -F: '{s+=$NF}END{print s+0}'||echo 0)
[[ $PE_FC -ge 8 ]] && chk "Phase E functions (>=8)" pass pass || chk "Phase E functions (>=8)" pass "fail($PE_FC)"

echo ""; echo "--- Phase D Runner Checks ---"
[[ -f "$VD/runner.js" ]] && chk "runner.js exists" yes yes || chk "runner.js exists" yes no
for f in runner.js; do node -c "$VD/$f" >/dev/null 2>&1 && chk "$f syntax" ok ok || chk "$f syntax" ok fail; done
RD_FC=$(grep -cE '(runnerCheckHealth|runnerGenerate|runnerLoadLibrary|runnerLoadSnapshot|runnerCompareSnapshot|runnerAddTimeline|renderSnapshotLibrary|runnerApi)' "$VD/runner.js" 2>/dev/null || echo 0)
[[ $RD_FC -ge 6 ]] && chk "runner functions (>=6)" pass pass || chk "runner functions (>=6)" pass "fail($RD_FC)"
grep -qF "runner-panel" "$VD/index.html" && chk "runner panel html" yes yes || chk "runner panel html" yes no
grep -qF "runner-mode-badge" "$VD/index.html" && chk "runner badge" yes yes || chk "runner badge" yes no
grep -qF "pickerPickDirectory" "$VD/runner.js" && chk "project picker folder chooser" yes yes || chk "project picker folder chooser" yes no
grep -qF "picker.chooseFolder" "$VD/index.html" && chk "choose folder i18n html" yes yes || chk "choose folder i18n html" yes no
grep -qF "runnerPickDirectory" "$VD/runner.js" && chk "workbench folder chooser" yes yes || chk "workbench folder chooser" yes no
grep -qF "runnerBrowse" "$VD/runner.js" && chk "workbench in-page browse" yes yes || chk "workbench in-page browse" yes no
grep -qF "extractProjectCandidates" "$VD/runner.js" && chk "candidate project parser" yes yes || chk "candidate project parser" yes no
grep -qF "runnerUseCandidate" "$VD/runner.js" && chk "candidate project action" yes yes || chk "candidate project action" yes no
grep -qF "projectInventory" "$VD/runner.js" && chk "project radar api client" yes yes || chk "project radar api client" yes no
grep -qF "renderProjectRadar" "$VD/runner.js" && chk "project radar renderer" yes yes || chk "project radar renderer" yes no
grep -qF "picker-project-radar" "$VD/index.html" && chk "project radar picker html" yes yes || chk "project radar picker html" yes no
grep -qF "runner-project-radar" "$VD/index.html" && chk "project radar runner html" yes yes || chk "project radar runner html" yes no
grep -qF ".project-radar" "$VD/styles.css" && chk "project radar css" yes yes || chk "project radar css" yes no
grep -qF "projectRadar.multiProject" "$VD/i18n.js" && chk "project radar i18n" yes yes || chk "project radar i18n" yes no
grep -qF "runner-browse-list" "$VD/index.html" && chk "workbench browse html" yes yes || chk "workbench browse html" yes no
grep -qF "restoreWorkbenchSnapshot" "$VD/runner.js" && chk "refresh restores snapshot" yes yes || chk "refresh restores snapshot" yes no
grep -qF "snapshot" "$VD/runner.js" && grep -qF "history.replaceState" "$VD/runner.js" && chk "snapshot url persistence" yes yes || chk "snapshot url persistence" yes no
grep -qF "rememberWorkbenchTab" "$VD/index.html" && chk "tab url persistence" yes yes || chk "tab url persistence" yes no

echo ""; echo "--- Graph Visual Checks ---"
grep -qF "graph-visual" "$VD/index.html" && chk "graph visual html" yes yes || chk "graph visual html" yes no
grep -qF "renderGraphVisual" "$VD/app.js" && chk "graph visual renderer" yes yes || chk "graph visual renderer" yes no
grep -qF "<svg" "$VD/app.js" && chk "graph svg renderer" yes yes || chk "graph svg renderer" yes no
grep -qF ".graph-visual" "$VD/styles.css" && chk "graph visual css" yes yes || chk "graph visual css" yes no
grep -qF "focusGraphNode" "$VD/app.js" && chk "graph drill function" yes yes || chk "graph drill function" yes no
grep -qF "graphNeighborIds" "$VD/app.js" && chk "graph neighbor function" yes yes || chk "graph neighbor function" yes no
grep -qF "graph-relation-row" "$VD/app.js" && chk "graph relation rows" yes yes || chk "graph relation rows" yes no
grep -qF ".graph-relation-grid" "$VD/styles.css" && chk "graph relation css" yes yes || chk "graph relation css" yes no
grep -qF "setGraphEdgeMode" "$VD/app.js" && chk "graph edge mode" yes yes || chk "graph edge mode" yes no
grep -qF "graph-depth-filter" "$VD/index.html" && chk "graph depth control" yes yes || chk "graph depth control" yes no
grep -qF "graph-layout-mode" "$VD/index.html" && chk "graph layout mode" yes yes || chk "graph layout mode" yes no
grep -qF "graph-layout-buttons" "$VD/index.html" && chk "graph segmented layouts" yes yes || chk "graph segmented layouts" yes no
grep -qF "setGraphLayout" "$VD/app.js" && chk "graph layout function" yes yes || chk "graph layout function" yes no
grep -qF "graph-engine-mode" "$VD/index.html" && chk "graph engine mode" yes yes || chk "graph engine mode" yes no
grep -qF "setGraphEngine" "$VD/app.js" && chk "graph engine function" yes yes || chk "graph engine function" yes no
grep -qF "graph-zoom-lock-btn" "$VD/index.html" && chk "graph wheel lock button" yes yes || chk "graph wheel lock button" yes no
grep -qF "toggleGraphZoomLock" "$VD/app.js" && chk "graph wheel lock function" yes yes || chk "graph wheel lock function" yes no
grep -qF "CodeLatticeG6Graph" "$VD/graph-g6.js" && chk "G6 adapter" yes yes || chk "G6 adapter" yes no
grep -qF "new Graph" "$VD/graph-g6.js" && chk "G6 graph constructor" yes yes || chk "G6 graph constructor" yes no
grep -qF "drag-canvas" "$VD/graph-g6.js" && chk "G6 drag canvas" yes yes || chk "G6 drag canvas" yes no
grep -qF "zoom-canvas" "$VD/graph-g6.js" && chk "G6 zoom canvas" yes yes || chk "G6 zoom canvas" yes no
grep -qF "zoomLocked" "$VD/graph-g6.js" && chk "G6 zoom lock" yes yes || chk "G6 zoom lock" yes no
grep -qF "uniqueNodes" "$VD/graph-g6.js" && chk "G6 duplicate node guard" yes yes || chk "G6 duplicate node guard" yes no
grep -qF "g6.min.js" "$VD/index.html" && chk "G6 script ref" yes yes || chk "G6 script ref" yes no
grep -qF "MIT" "$VD/vendor/g6/LICENSE" && chk "G6 license" yes yes || chk "G6 license" yes no
grep -qF "graph.engineG6" "$VD/i18n.js" && chk "G6 i18n" yes yes || chk "G6 i18n" yes no
grep -qF "graph.zoomLocked" "$VD/i18n.js" && chk "graph zoom i18n" yes yes || chk "graph zoom i18n" yes no
grep -qF ".graph-g6-host" "$VD/styles.css" && chk "G6 css" yes yes || chk "G6 css" yes no
grep -qF "graph-layout-blueprint" "$VD/styles.css" && chk "graph blueprint style" yes yes || chk "graph blueprint style" yes no
grep -qF "graph.layoutGalaxy" "$VD/i18n.js" && chk "graph layout i18n" yes yes || chk "graph layout i18n" yes no
grep -qF "toggleGraphPosterMode" "$VD/app.js" && chk "graph poster mode" yes yes || chk "graph poster mode" yes no

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
