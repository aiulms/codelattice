#!/usr/bin/env bash
# MCP v0.1 Dogfood — real stdio JSON-RPC against the MCP server.
# Exercises all 8 tools and reports pass/fail per tool.
#
# Usage: bash scripts/mcp-dogfood.sh [path-to-fixture]
# Default fixture: fixtures/call-resolution/c1-same-module

set -euo pipefail

FIXTURE="${1:-fixtures/call-resolution/c1-same-module}"
FIXTURE_ABS="$(cd "$(dirname "$0")/.." && pwd)/$FIXTURE"

# Build the binary first
echo "--- Building ---"
cargo build -p gitnexus-rust-core-cli --quiet 2>/dev/null
BIN="$(cd "$(dirname "$0")/.." && pwd)/target/debug/gitnexus-rust-core-cli"

echo "--- MCP v0.1 Dogfood ---"
echo "Binary: $BIN"
echo "Fixture: $FIXTURE_ABS"
echo ""

# Helper: send a JSON-RPC request and read one response line
ID=1
send_and_recv() {
    local method="$1"
    local params="$2"
    local req
    req=$(printf '{"jsonrpc":"2.0","id":%d,"method":"%s","params":%s}' "$ID" "$method" "$params")
    echo "$req" >&2
    echo "$req"
    ID=$((ID + 1))
}

PASS=0
FAIL=0
RESULTS=()

check_tool() {
    local tool_name="$1"
    local args="$2"
    local check_expr="$3"

    local request
    request=$(printf '{"jsonrpc":"2.0","id":%d,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$ID" "$tool_name" "$args")
    ID=$((ID + 1))

    local response
    response=$(echo "$request" | "$BIN" mcp 2>/dev/null | head -1)

    if [ -z "$response" ]; then
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: $tool_name — no response")
        return
    fi

    local is_error
    is_error=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('result',{}).get('isError',False))" 2>/dev/null || echo "True")

    if [ "$is_error" = "True" ]; then
        FAIL=$((FAIL + 1))
        local err_msg
        err_msg=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; print(json.loads(t).get('message','unknown'))" 2>/dev/null || echo "unknown error")
        RESULTS+=("FAIL: $tool_name — $err_msg")
        return
    fi

    # Run check expression if provided
    if [ -n "$check_expr" ]; then
        local check_result
        check_result=$(echo "$response" | python3 -c "
import json, sys
d = json.load(sys.stdin)
text = d['result']['content'][0]['text']
data = json.loads(text)
result = $check_expr
print('PASS' if result else 'FAIL')
" 2>/dev/null || echo "FAIL")

        if [ "$check_result" = "PASS" ]; then
            PASS=$((PASS + 1))
            RESULTS+=("PASS: $tool_name")
        else
            FAIL=$((FAIL + 1))
            RESULTS+=("FAIL: $tool_name — check expression failed")
        fi
    else
        PASS=$((PASS + 1))
        RESULTS+=("PASS: $tool_name")
    fi
}

# ============================================================
# 1. Initialize
# ============================================================
echo "1. initialize"
INIT_REQ='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"dogfood","version":"1.0"}}}'
INIT_RESP=$(echo "$INIT_REQ" | "$BIN" mcp 2>/dev/null | head -1)
if echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['result']['serverInfo']['name']=='codelattice'" 2>/dev/null; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: initialize")
    echo "   → server name: codelattice"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: initialize")
    echo "   → unexpected response"
fi
ID=2

# ============================================================
# 2. tools/list
# ============================================================
echo "2. tools/list"
TL_REQ=$(printf '{"jsonrpc":"2.0","id":2,"method":"tools/list"}')
TL_RESP=$(echo "$TL_REQ" | "$BIN" mcp 2>/dev/null | head -1)
TOOL_COUNT=$(echo "$TL_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))" 2>/dev/null || echo "0")
if [ "$TOOL_COUNT" = "8" ]; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: tools/list (8 tools)")
    echo "   → 8 tools listed"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: tools/list (expected 8, got $TOOL_COUNT)")
    echo "   → expected 8 tools, got $TOOL_COUNT"
fi
ID=3

# ============================================================
# 3-8. Call each tool via separate invocations
# ============================================================
echo "3. codelattice_analyze"
check_tool "codelattice_analyze" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"strict\":false,\"includeGraph\":false}" \
    "data.get('language') == 'rust' and data.get('summary') is not None"

echo "4. codelattice_quality"
check_tool "codelattice_quality" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('overall') == 'pass'"

echo "5. codelattice_summary"
check_tool "codelattice_summary" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('graphSummary') is not None"

echo "6. codelattice_graph_overview"
check_tool "codelattice_graph_overview" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('nodeCount', 0) > 0 and data.get('symbolCount', 0) > 0"

echo "7. codelattice_symbol_search"
check_tool "codelattice_symbol_search" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"query\":\"helper\"}" \
    "data.get('matchCount', 0) > 0"

echo "8. codelattice_unresolved_report"
check_tool "codelattice_unresolved_report" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('supported') == True"

# ============================================================
# Summary
# ============================================================
echo ""
echo "============================================"
echo " MCP v0.1 Dogfood Results"
echo "============================================"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo ""
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo ""

if [ "$FAIL" -eq 0 ]; then
    echo "All checks passed — MCP v0.1 dogfood successful."
    exit 0
else
    echo "Some checks failed — see above for details."
    exit 1
fi
