#!/usr/bin/env bash
set -euo pipefail
SD="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$SD/.." && pwd)"
VD="$WS/webui/snapshot-viewer"
P=0; F=0
pass(){ P=$((P+1)); echo "  [PASS] $1"; }
fail(){ F=$((F+1)); echo "  [FAIL] $1"; }
echo "CodeLattice i18n Smoke (Phase I)"
# File exists + syntax
[[ -f "$VD/i18n.js" ]] && pass "i18n.js exists" || fail "i18n.js missing"
node -c "$VD/i18n.js" >/dev/null 2>&1 && pass "i18n.js syntax OK" || fail "i18n.js syntax"
# Contains zh and en messages
grep -q '"zh"' "$VD/i18n.js" && pass "zh messages" || fail "zh messages"
grep -q '"en"' "$VD/i18n.js" && pass "en messages" || fail "en messages"
# Key translations exist
for k in tab.dashboard tab.explore tab.graph tab.cleanup tab.release tab.workflows tab.diff tab.timeline tab.report picker.analyze picker.loadJson caution.staticOnly report.generate live.run guided.scenarios; do
  grep -q "\"$k\"" "$VD/i18n.js" && pass "key: $k" || fail "key: $k"
done
# Language toggle in HTML
grep -q "i18n-toggle" "$VD/index.html" && pass "lang toggle" || fail "lang toggle"
grep -q "data-i18n=" "$VD/index.html" && pass "data-i18n attrs" || fail "data-i18n attrs"
# Picker UI
grep -q "picker-recent" "$VD/index.html" && pass "picker ui" || fail "picker ui"
grep -q "quick-analyze" "$VD/runner.js" && pass "quick-analyze" || fail "quick-analyze"
T=$((P+F))
echo ""; echo "=== i18n: $P passed, $F failed, $T total ==="
[[ $F -gt 0 ]] && exit 1
echo "I18N SMOKE PASSED"
