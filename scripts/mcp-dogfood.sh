#!/usr/bin/env bash
# MCP dogfood — real stdio JSON-RPC against the MCP server.
# Exercises the low-level tool surface, facade tools, source snippets, cache
# behavior, doc association, workspace graph, and cross-project impact.
#
# Usage: bash scripts/mcp-dogfood.sh [path-to-fixture]
# Default fixture: fixtures/call-resolution/c1-same-module

set -euo pipefail

FIXTURE="${1:-fixtures/call-resolution/c1-same-module}"
FIXTURE_ABS="$(cd "$(dirname "$0")/.." && pwd)/$FIXTURE"
WORKSPACE_FIXTURE_ABS="$(cd "$(dirname "$0")/.." && pwd)/fixtures/workspace"

# Build the binary first with all optional language adapters for full profile coverage.
echo "--- Building ---"
cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python --bins --quiet 2>/dev/null
BIN="$(cd "$(dirname "$0")/.." && pwd)/target/debug/codelattice"
export CODELATTICE_MCP_TOOLSET=full

echo "--- MCP v0.12 Dogfood ---"
echo "Binary: $BIN"
echo "Fixture: $FIXTURE_ABS"
echo ""

# --- Profile detection ---
PROFILE_RESP=$(echo '{"jsonrpc":"2.0","id":999,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"dogfood","version":"1.0"}}}' | "$BIN" mcp 2>/dev/null | head -1)
PROFILE_VER=$(echo "$PROFILE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo']['version'])" 2>/dev/null || echo "unknown")
PROFILE_CJ=$(echo "$PROFILE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('cangjieSupport','unknown'))" 2>/dev/null || echo "unknown")
PROFILE_TOOLS=$(echo "$PROFILE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('toolCount','unknown'))" 2>/dev/null || echo "unknown")
echo "Profile: version=$PROFILE_VER cangjie=$PROFILE_CJ tools=$PROFILE_TOOLS"
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
if [ "$TOOL_COUNT" -ge 49 ]; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: tools/list ($TOOL_COUNT tools)")
    echo "   → $TOOL_COUNT tools listed"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: tools/list (expected >= 49, got $TOOL_COUNT)")
    echo "   → expected >= 49 tools, got $TOOL_COUNT"
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

echo "9. codelattice_export_bridge"
check_tool "codelattice_export_bridge" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('schemaVersion') is not None and data.get('symbols', 0) > 0"

# ============================================================
# v0.2: Local Graph Intelligence (tools 10-17)
# ============================================================

echo "10. codelattice_symbol_context (with snippet)"
check_tool "codelattice_symbol_context" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"name\":\"helper\"}" \
    "data.get('matchCount', 0) > 0 and len(data.get('candidates', [])) > 0 and data['candidates'][0].get('name') == 'helper' and data['candidates'][0].get('sourceSnippet', {}).get('lines') is not None"

echo "11. codelattice_calls_from"
check_tool "codelattice_calls_from" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"symbol\":\"main_fn\",\"depth\":1}" \
    "data.get('symbol') == 'main_fn' and data.get('callCount', 0) >= 0"

echo "12. codelattice_calls_to"
check_tool "codelattice_calls_to" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"symbol\":\"helper\",\"depth\":1}" \
    "data.get('symbol') == 'helper' and data.get('callerCount', 0) >= 0"

echo "13. codelattice_impact_preview"
check_tool "codelattice_impact_preview" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"symbol\":\"helper\",\"depth\":1}" \
    "data.get('symbol') == 'helper' and data.get('risk') in ['LOW','MEDIUM','HIGH'] and isinstance(data.get('riskReasons'), list) and isinstance(data.get('impactMetrics'), dict) and isinstance(data.get('confidenceSummary'), dict) and isinstance(data.get('reviewFocus'), dict)"

echo "14. codelattice_query_graph"
check_tool "codelattice_query_graph" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"nodeKind\":\"function\"}" \
    "data.get('nodeCount', 0) >= 0"

echo "15. codelattice_project_overview"
check_tool "codelattice_project_overview" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('language') == 'rust' and data.get('nodeCount', 0) > 0"

echo "16. codelattice_repo_registry"
check_tool "codelattice_repo_registry" \
    "{\"action\":\"status\",\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('action') == 'status' and data.get('indexed') == True"

echo "17. codelattice_rename_preview"
check_tool "codelattice_rename_preview" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"symbol\":\"helper\",\"newName\":\"assist\"}" \
    "data.get('symbol') == 'helper' and data.get('applySupported') == False"

# ============================================================
# v0.3: Process-Local Cache (tools 18-19, plus cache hit verification)
# ============================================================

echo "18. codelattice_cache_status (empty)"
check_tool "codelattice_cache_status" \
    "{}" \
    "data.get('memory',{}).get('entryCount') == 0 and data.get('memory',{}).get('totalHits') == 0"

echo "19. codelattice_cache_clear"
check_tool "codelattice_cache_clear" \
    "{}" \
    "data.get('clearedCount') == 0"

# ============================================================
# v0.3: Cache hit verification (multi-request session)
# ============================================================
echo "20. cache hit verification (analyze x2 in one session)"
CACHE_SESSION_REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_analyze","arguments":{"root":"%s","language":"rust"}}}\n' "$FIXTURE_ABS" "$FIXTURE_ABS")
CACHE_SESSION_RESP=$(echo "$CACHE_SESSION_REQ" | "$BIN" mcp 2>/dev/null)
CACHE_HIT_RESULT=$(echo "$CACHE_SESSION_RESP" | python3 -c "
import json, sys
lines = sys.stdin.read().strip().split('\n')
if len(lines) >= 2:
    d1 = json.loads(lines[0])
    d2 = json.loads(lines[1])
    t1 = json.loads(d1['result']['content'][0]['text'])
    t2 = json.loads(d2['result']['content'][0]['text'])
    miss = t1.get('cacheHit') == False
    hit = t2.get('cacheHit') == True
    print('PASS' if (miss and hit) else 'FAIL')
else:
    print('FAIL')
" 2>/dev/null || echo "FAIL")
if [ "$CACHE_HIT_RESULT" = "PASS" ]; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: cache hit verification (miss→hit)")
    echo "   → first call: miss, second call: hit ✓"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: cache hit verification")
    echo "   → cache hit not detected"
fi

# ============================================================
# v0.6: cache_prewarm
# ============================================================
echo "21. codelattice_cache_prewarm"
check_tool "codelattice_cache_prewarm" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\"}" \
    "data.get('warmed') == True and isinstance(data.get('summary'), dict)"

# ============================================================
# v0.7: Profile check
# ============================================================
echo "22. profile cangjie support"
if [[ "$PROFILE_CJ" == "True" ]]; then
    PASS=$((PASS + 1))
    RESULTS+=("PASS: cangjieSupport=$PROFILE_CJ")
    echo "   → cangjieSupport=$PROFILE_CJ ✓"
else
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL: cangjieSupport=$PROFILE_CJ (expected True)")
    echo "   → cangjieSupport=$PROFILE_CJ (expected True)"
fi

# ============================================================
# v0.8: Large Project Insight
# ============================================================
echo "23. codelattice_project_insights"
check_tool "codelattice_project_insights" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true}" \
    "isinstance(data.get('summary'), dict) and data['summary'].get('language') == 'rust' and isinstance(data.get('hotspotFiles'), list) and isinstance(data.get('hotspotSymbols'), list) and isinstance(data.get('readFirst'), list) and isinstance(data.get('reviewFirst'), list) and data.get('generatedFrom', {}).get('graphBased') == True"

# ============================================================
# v0.9: AI Review Plan
# ============================================================
echo "24. codelattice_review_plan (onboarding)"
check_tool "codelattice_review_plan" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"onboarding\"}" \
    "isinstance(data.get('summary'), dict) and data['mode'] == 'onboarding' and isinstance(data.get('readPlan'), list) and isinstance(data.get('riskReviewPlan'), list) and isinstance(data.get('recommendedMcpCalls'), list) and data.get('generatedFrom', {}).get('graphBased') == True"

# ============================================================
# v0.10: Dead Code Candidates
# ============================================================
echo "25. codelattice_dead_code_candidates"
check_tool "codelattice_dead_code_candidates" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"limit\":10}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('deletionSafe') == False"

# ============================================================
# v0.19: Graph Diagnostics Pack (5 tools)
# ============================================================
echo "26. codelattice_impact_analysis"
check_tool "codelattice_impact_analysis" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"target\":\"helper\",\"compact\":true}" \
    "isinstance(data.get('targetMatched'), dict) and data.get('generatedFrom', {}).get('staticAnalysisOnly') == True"

echo "27. codelattice_risk_hotspots"
check_tool "codelattice_risk_hotspots" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"maxResults\":5}" \
    "isinstance(data.get('summary'), dict) and isinstance(data.get('hotspotSymbols'), list)"

echo "28. codelattice_architecture_drift"
check_tool "codelattice_architecture_drift" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('heuristic') == True"

echo "29. codelattice_ai_context_pack"
check_tool "codelattice_ai_context_pack" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"task\":\"helper function\",\"compact\":true}" \
    "isinstance(data.get('contextFiles'), list) and isinstance(data.get('keySymbols'), list)"

echo "30. codelattice_review_gate"
check_tool "codelattice_review_gate" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"useGitDiff\":false,\"changedFiles\":[\"src/lib.rs\"],\"compact\":true}" \
    "isinstance(data.get('touchedSymbols'), list) and data.get('generatedFrom', {}).get('compilerVerified') == False"

# ============================================================
# v0.20: Entry Point & Reachability Pack
# ============================================================
echo "31. codelattice_reachability_map"
check_tool "codelattice_reachability_map" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('runtimeVerified') == False and isinstance(data.get('entryPoints'), list)"

# ============================================================
# v0.21: External API Surface / Public API Caution
# ============================================================
echo "32. codelattice_external_api_surface"
echo "33. codelattice_framework_entry_hints"
echo "34. codelattice_breaking_change_review"
echo "35. codelattice_consistency_review"
echo "36. codelattice_config_examples_review"
echo "37. codelattice_automation_graph"
echo "38. codelattice_workflow_presets"
check_tool "codelattice_external_api_surface" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('externalUsageVerified') == False and 'cautionLevel' in json.dumps(data.get('externalSurfaceSymbols', []))"

check_tool "codelattice_framework_entry_hints" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"limit\":10}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('heuristic') == True"

# ============================================================
# Summary
# ============================================================
check_tool "codelattice_workflow_presets" \
    "{\"scenario\":\"delete_code\",\"compact\":false}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('presetOnly') == True"

check_tool "codelattice_config_examples_review" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"limit\":5}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('scriptsExecuted') == False"

check_tool "codelattice_automation_graph" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"limit\":10}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('scriptsExecuted') == False and data.get('generatedFrom', {}).get('runtimeVerified') == False"

check_tool "codelattice_consistency_review" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"changedSymbols\":[\"main\"],\"limit\":5}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('heuristic') == True"

check_tool "codelattice_breaking_change_review" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"compact\":true,\"changedSymbols\":[\"main\"],\"limit\":5}" \
    "isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('heuristic') == True"

# ============================================================
# v0.28-v0.29: Workspace + facade surface
# ============================================================
echo "39. codelattice_workspace_graph"
check_tool "codelattice_workspace_graph" \
    "{\"root\":\"$WORKSPACE_FIXTURE_ABS\",\"compact\":true}" \
    "data.get('schemaVersion') == 'workspace.graph.v1' and isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('staticAnalysis') == True"

echo "40. codelattice_cross_project_impact"
check_tool "codelattice_cross_project_impact" \
    "{\"root\":\"$WORKSPACE_FIXTURE_ABS\",\"target\":{\"path\":\"Dockerfile\"},\"compact\":true}" \
    "data.get('schemaVersion') == 'workspace.impact.v1' and isinstance(data.get('summary'), dict) and data.get('generatedFrom', {}).get('staticAnalysis') == True"

echo "41. codelattice_project facade"
check_tool "codelattice_project" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"overview\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_project' and 'codelattice_project_overview' in data.get('underlyingTools', [])"

echo "42. codelattice_symbol facade"
check_tool "codelattice_symbol" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"search\",\"query\":\"helper\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_symbol' and 'codelattice_symbol_search' in data.get('underlyingTools', [])"

echo "43. codelattice_change_review facade"
check_tool "codelattice_change_review" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"impact\",\"symbol\":\"helper\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_change_review' and 'codelattice_impact_preview' in data.get('underlyingTools', [])"

echo "44. codelattice_cleanup facade"
check_tool "codelattice_cleanup" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"dead_code\",\"compact\":true,\"limit\":5}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_cleanup' and 'codelattice_dead_code_candidates' in data.get('underlyingTools', [])"

echo "45. codelattice_workspace facade"
check_tool "codelattice_workspace" \
    "{\"root\":\"$WORKSPACE_FIXTURE_ABS\",\"mode\":\"graph\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_workspace' and 'codelattice_workspace_graph' in data.get('underlyingTools', [])"

echo "46. codelattice_release_check facade"
check_tool "codelattice_release_check" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"mode\":\"quick\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_release_check' and 'codelattice_quality' in data.get('underlyingTools', [])"

echo "47. codelattice_cache facade"
check_tool "codelattice_cache" \
    "{\"mode\":\"status\",\"compact\":true}" \
    "data.get('schemaVersion') == 'facade.v1' and data.get('tool') == 'codelattice_cache' and 'codelattice_cache_status' in data.get('underlyingTools', [])"

echo "48. codelattice_workflow facade"
check_tool "codelattice_workflow" \
    "{\"mode\":\"onboarding\",\"compact\":true}" \
    "data.get('schemaVersion') == 'ai.workflow.v1' and data.get('tool') == 'codelattice_workflow' and data.get('mode') == 'onboarding' and isinstance(data.get('nextActions'), list) and len(data.get('nextActions')) > 0"

echo "49. codelattice_root_cause_assistant"
check_tool "codelattice_root_cause_assistant" \
    "{\"root\":\"$FIXTURE_ABS\",\"language\":\"rust\",\"issue\":\"helper returns an unexpected value after a caller change\",\"availableCapabilities\":[\"read_code\",\"read_git_diff\",\"edit_code\"],\"compact\":true,\"limit\":5}" \
    "data.get('schemaVersion') == 'rootCauseEvidence.v1' and data.get('permissionSummary', {}).get('mode') == 'capability-aware' and data.get('generatedFrom', {}).get('runtimeVerified') == False and isinstance(data.get('rootCauseHypotheses'), list) and isinstance(data.get('missingEvidence'), list)"


echo ""
echo "============================================"
echo " MCP v0.9 Dogfood Results"
echo "============================================"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo ""
echo "  PASS: $PASS"
echo "  FAIL: $FAIL"
echo ""

if [ "$FAIL" -eq 0 ]; then
    echo "All checks passed — MCP v0.12 dogfood successful."
    exit 0
else
    echo "Some checks failed — see above for details."
    exit 1
fi
