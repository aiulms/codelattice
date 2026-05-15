#!/usr/bin/env bash
# project-insights-smoke.sh — smoke test for codelattice_project_insights tool
#
# Validates the large project insight MCP tool returns structured output
# with all expected sections: summary, hotspots, risk map, read-first, review-first.
#
# Usage: bash scripts/project-insights-smoke.sh [path-to-fixture]
# Default fixture: fixtures/call-resolution/c1-same-module

set -euo pipefail

FIXTURE="${1:-fixtures/call-resolution/c1-same-module}"
FIXTURE_ABS="$(cd "$(dirname "$0")/.." && pwd)/$FIXTURE"

echo "--- Building ---"
cargo build -p gitnexus-rust-core-cli --bins --quiet 2>/dev/null
BIN="$(cd "$(dirname "$0")/.." && pwd)/target/debug/codelattice"

echo "--- Project Insights Smoke ---"
echo "Binary: $BIN"
echo "Fixture: $FIXTURE_ABS"
echo ""

# Helper: call a tool and parse result
call_tool() {
    local tool_name="$1"
    local args="$2"

    local request
    request=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$tool_name" "$args")

    echo "$request" | "$BIN" mcp 2>/dev/null | head -1
}

PASS=0
FAIL=0

check() {
    local label="$1"
    local actual="$2"
    local expected="$3"

    if [ "$actual" = "$expected" ]; then
        echo "  PASS: $label"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $label (expected=$expected, got=$actual)"
        FAIL=$((FAIL + 1))
    fi
}

# --- Test 1: Compact mode ---
echo "1. Compact mode"
RESP=$(call_tool "codelattice_project_insights" "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true}")
LANG=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(t['summary']['language'])" 2>/dev/null || echo "FAIL")
check "language" "$LANG" "rust"

COMPACT=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(t.get('compact',False))" 2>/dev/null || echo "FAIL")
check "compact=true" "$COMPACT" "True"

GRAPH_BASED=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(t['generatedFrom']['graphBased'])" 2>/dev/null || echo "FAIL")
check "graphBased" "$GRAPH_BASED" "True"

COMPILER_VERIFIED=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(t['generatedFrom']['compilerVerified'])" 2>/dev/null || echo "FAIL")
check "compilerVerified=false" "$COMPILER_VERIFIED" "False"

# --- Test 2: Sections exist ---
echo "2. Sections exist"
for section in entryPointCandidates hotspotFiles hotspotSymbols riskMap readFirst reviewFirst docsSignals; do
    EXISTS=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); val=t.get('$section'); print('ok' if isinstance(val, (list,dict)) else 'missing')" 2>/dev/null || echo "FAIL")
    check "$section exists" "$EXISTS" "ok"
done

# --- Test 3: Low confidence zones ---
echo "3. Low confidence zones structure"
LCZ=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); lz=t.get('lowConfidenceZones',{}); print('ok' if isinstance(lz.get('fileZones'),list) and isinstance(lz.get('symbolZones'),list) else 'bad')" 2>/dev/null || echo "FAIL")
check "lowConfidenceZones" "$LCZ" "ok"

# --- Test 4: Full mode ---
echo "4. Full mode"
RESP2=$(call_tool "codelattice_project_insights" "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":false}")
FILE_METRICS=$(echo "$RESP2" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print('ok' if isinstance(t.get('fileMetrics'),list) else 'missing')" 2>/dev/null || echo "FAIL")
check "fileMetrics" "$FILE_METRICS" "ok"

COMPACT2=$(echo "$RESP2" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(t.get('compact',True))" 2>/dev/null || echo "FAIL")
check "compact=false" "$COMPACT2" "False"

# --- Test 5: Limit parameter ---
echo "5. Limit parameter"
RESP3=$(call_tool "codelattice_project_insights" "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"limit\":2}")
HS_LEN=$(echo "$RESP3" | python3 -c "import json,sys; d=json.load(sys.stdin); t=json.loads(d['result']['content'][0]['text']); print(len(t.get('hotspotSymbols',[])))" 2>/dev/null || echo "99")
check "hotspotSymbols <= 2" "$([ "$HS_LEN" -le 2 ] && echo ok || echo fail)" "ok"

# --- Summary ---
echo ""
echo "============================================"
echo " Project Insights Smoke Results"
 echo "============================================"
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo ""

if [ "$FAIL" -eq 0 ]; then
    echo "All checks passed — project insights smoke successful."
    exit 0
else
    echo "Some checks failed — see above for details."
    exit 1
fi
