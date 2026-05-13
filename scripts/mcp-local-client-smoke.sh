#!/usr/bin/env bash
# MCP Local Client Integration Smoke — simulates an MCP client using the wrapper.
#
# Tests that the wrapper script can start the server, accept JSON-RPC calls,
# and return valid responses for v0.5 tools (20 tools including cache).
#
# Usage: bash scripts/mcp-local-client-smoke.sh
#
# Exits 0 on success, 1 on failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WRAPPER="$SCRIPT_DIR/codelattice-mcp.sh"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE="$REPO_ROOT/fixtures/call-resolution/c1-same-module"

PASS=0
FAIL=0
RESULTS=()

echo "--- MCP Local Client Integration Smoke ---"
echo "Wrapper: $WRAPPER"
echo "Fixture: $FIXTURE"
echo "Repo:    $REPO_ROOT"
echo ""

# Verify wrapper exists
if [[ ! -f "$WRAPPER" ]]; then
    echo "FAIL: wrapper not found at $WRAPPER"
    exit 1
fi

# Helper: call a tool via wrapper (separate invocation per tool, like a real MCP client)
call_tool() {
    local tool_name="$1"
    local args="$2"

    local request
    request=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$tool_name" "$args")

    local response
    response=$(echo "$request" | bash "$WRAPPER" 2>/dev/null | head -1)

    echo "$response"
}

# Helper: check response
check_response() {
    local label="$1"
    local response="$2"
    local check_expr="$3"

    if [ -z "$response" ]; then
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: $label — no response")
        return
    fi

    # Check for MCP error
    local is_error
    is_error=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('result',{}).get('isError',False))" 2>/dev/null || echo "True")

    if [ "$is_error" = "True" ]; then
        FAIL=$((FAIL + 1))
        local err_msg
        err_msg=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; print(json.loads(t).get('message','unknown'))" 2>/dev/null || echo "unknown error")
        RESULTS+=("FAIL: $label — $err_msg")
        return
    fi

    # Run check expression
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
            RESULTS+=("PASS: $label")
        else
            FAIL=$((FAIL + 1))
            RESULTS+=("FAIL: $label — check expression failed")
        fi
    else
        PASS=$((PASS + 1))
        RESULTS+=("PASS: $label")
    fi
}

# ============================================================
# 1. Initialize
# ============================================================
echo "1. initialize"
INIT_REQ='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"local-smoke","version":"1.0"}}}'
INIT_RESP=$(echo "$INIT_REQ" | bash "$WRAPPER" 2>/dev/null | head -1)
if echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['result']['serverInfo']['name']=='codelattice'" 2>/dev/null; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: initialize (via wrapper)")
    echo "   → server name: codelattice"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: initialize")
    echo "   → unexpected response: $(echo "$INIT_RESP" | head -c 200)"
fi

# ============================================================
# 2. tools/list
# ============================================================
echo "2. tools/list"
TL_RESP=$(echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | bash "$WRAPPER" 2>/dev/null | head -1)
TOOL_COUNT=$(echo "$TL_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))" 2>/dev/null || echo "0")
if [ "$TOOL_COUNT" -ge 20 ]; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: tools/list ($TOOL_COUNT tools >= 20)")
    echo "   → $TOOL_COUNT tools listed"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: tools/list (expected >= 20, got $TOOL_COUNT)")
    echo "   → expected >= 20 tools, got $TOOL_COUNT"
fi

# ============================================================
# 3. codelattice_project_overview (on fixture)
# ============================================================
echo "3. codelattice_project_overview"
RESP=$(call_tool "codelattice_project_overview" "{\"root\":\"$FIXTURE\",\"language\":\"rust\"}")
check_response "project_overview (fixture)" "$RESP" \
    "data.get('language') == 'rust' and data.get('nodeCount', 0) > 0"

# ============================================================
# 4. codelattice_symbol_context (on fixture)
# ============================================================
echo "4. codelattice_symbol_context"
RESP=$(call_tool "codelattice_symbol_context" "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"name\":\"helper\"}")
check_response "symbol_context (helper)" "$RESP" \
    "data.get('matchCount', 0) > 0"

# ============================================================
# 5. codelattice_calls_from (on fixture)
# ============================================================
echo "5. codelattice_calls_from"
RESP=$(call_tool "codelattice_calls_from" "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"symbol\":\"main_fn\",\"depth\":1}")
check_response "calls_from (main_fn)" "$RESP" \
    "data.get('symbol') == 'main_fn'"

# ============================================================
# 6. codelattice_impact_preview (on fixture)
# ============================================================
echo "6. codelattice_impact_preview"
RESP=$(call_tool "codelattice_impact_preview" "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"symbol\":\"helper\",\"depth\":1}")
check_response "impact_preview (helper)" "$RESP" \
    "data.get('symbol') == 'helper' and data.get('risk') in ['LOW','MEDIUM','HIGH'] and isinstance(data.get('riskReasons'), list) and isinstance(data.get('impactMetrics'), dict)"

# ============================================================
# 7. codelattice_rename_preview (on fixture)
# ============================================================
echo "7. codelattice_rename_preview"
RESP=$(call_tool "codelattice_rename_preview" "{\"root\":\"$FIXTURE\",\"language\":\"rust\",\"symbol\":\"helper\",\"newName\":\"assist\"}")
check_response "rename_preview (helper→assist)" "$RESP" \
    "data.get('symbol') == 'helper' and data.get('applySupported') == False"

# ============================================================
# 8. codelattice_project_overview (on CodeLattice self — larger project)
# NOTE: The full repo analysis is slow (~60-90s) via the MCP subprocess
# through cargo run. This is a known limitation documented in the
# experience report. Marked as optional — skipped in this smoke.
# The fixture-based project_overview (test #3) already validates the tool.
# ============================================================
echo "8. codelattice_project_overview (self-analysis) [optional, skipped — ~90s via cargo run]"
RESULTS+=("SKIP: project_overview (self) — too slow for smoke, fixture test covers this")

# ============================================================
# 9-10. v0.3 Cache tools
# ============================================================
echo "9. codelattice_cache_status"
RESP=$(call_tool "codelattice_cache_status" "{}")
check_response "cache_status (empty)" "$RESP" \
    "data.get('entryCount') is not None and data.get('totalHits') == 0"

echo "10. codelattice_cache_clear"
RESP=$(call_tool "codelattice_cache_clear" "{}")
check_response "cache_clear" "$RESP" \
    "data.get('clearedCount') is not None"

# ============================================================
# Summary
# ============================================================
echo ""
echo "============================================"
echo " Local Client Integration Smoke Results"
echo "============================================"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo ""
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo ""

# Cleanup check — no residual files created by smoke
# Note: .claude/ is injected by GitNexus Tool, not by MCP smoke — skip it
RESIDUAL=0
for f in CLAUDE.md .sisyphus; do
    if [[ -e "$REPO_ROOT/$f" ]]; then
        echo "WARN: residual file found: $f"
        RESIDUAL=$((RESIDUAL + 1))
    fi
done
# Check for temp JSON in /tmp (from export_bridge if tested)
TEMP_BRIDGE=$(ls /tmp/codelattice-bridge-*.json 2>/dev/null | head -1 || true)
if [[ -n "$TEMP_BRIDGE" ]]; then
    echo "NOTE: temp bridge file exists: $TEMP_BRIDGE (pre-existing, not from this smoke)"
fi

if [ "$FAIL" -eq 0 ] && [ "$RESIDUAL" -eq 0 ]; then
    echo "All checks passed — MCP local client integration smoke successful."
    exit 0
else
    echo "Some checks failed — see above for details."
    exit 1
fi
