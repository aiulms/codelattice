#!/usr/bin/env bash
# mcp-real-client-dry-run.sh — Simulate real MCP client calls without modifying config.
#
# Uses the MCP wrapper to call 10 high-frequency tools and report PASS/FAIL.
# Does NOT modify any real client configuration files.
#
# Usage: bash scripts/mcp-real-client-dry-run.sh [root_dir]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="${1:-$REPO_ROOT/fixtures/call-resolution/c1-same-module}"

# Find binary
BIN=""
for candidate in \
    "$REPO_ROOT/target/release/gitnexus-rust-core-cli" \
    "$REPO_ROOT/target/debug/gitnexus-rust-core-cli"; do
    if [[ -x "$candidate" ]]; then
        BIN="$candidate"
        break
    fi
done

if [[ -z "$BIN" ]]; then
    echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli"
    exit 1
fi

TMPFILE="/tmp/codelattice-dry-run-resp-$$.txt"

echo "=== CodeLattice MCP Real Client Dry-Run ==="
echo "Root: $ROOT"
echo "Bin:  $BIN"
echo ""

# Build multi-request sequence
printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"dry-run","version":"1.0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
    '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}' \
    "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_analyze\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_graph_overview\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_symbol_context\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\",\"name\":\"helper\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_calls_from\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\",\"symbol\":\"main\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_impact_preview\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\",\"symbol\":\"helper\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_production_assist\",\"arguments\":{\"root\":\"$ROOT\",\"language\":\"rust\"}}}" \
    "{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"codelattice_cache_status\",\"arguments\":{}}}" \
    | "$BIN" mcp 2>/dev/null > "$TMPFILE"

# Parse results
PASS=0
FAIL=0

check_result() {
    local name="$1" ok="$2"
    if [[ "$ok" == "true" ]]; then
        echo "  PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $name"
        FAIL=$((FAIL + 1))
    fi
}

# 1. Initialize
INIT_NAME=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 1:
                print(d['result']['serverInfo']['name'])
                break
        except: pass
" 2>/dev/null || echo "")
check_result "initialize handshake" "$([[ "$INIT_NAME" == "codelattice" ]] && echo true || echo false)"

# 2. tools/list
TOOL_COUNT=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 2:
                print(len(d['result']['tools']))
                break
        except: pass
" 2>/dev/null || echo "0")
check_result "tools/list ($TOOL_COUNT tools)" "$([[ "$TOOL_COUNT" -ge 20 ]] && echo true || echo false)"

# 3. cache_status (empty)
CACHE_EMPTY=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 3:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if t.get('entryCount') == 0 and 'maxEntries' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "cache_status (empty)" "$([[ "$CACHE_EMPTY" == "yes" ]] && echo true || echo false)"

# 4. analyze (miss)
ANALYZE_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 4:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if t.get('cacheHit') == False and ('nodeCount' in t or 'summary' in t) else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_analyze (miss)" "$([[ "$ANALYZE_OK" == "yes" ]] && echo true || echo false)"

# 5. graph_overview
OVERVIEW_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 5:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if 'nodeCount' in t or 'nodeKindCounts' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_graph_overview" "$([[ "$OVERVIEW_OK" == "yes" ]] && echo true || echo false)"

# 6. symbol_context
SYM_CTX_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 6:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if 'selected' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_symbol_context" "$([[ "$SYM_CTX_OK" == "yes" ]] && echo true || echo false)"

# 7. calls_from
CALLS_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 7:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if 'edges' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_calls_from" "$([[ "$CALLS_OK" == "yes" ]] && echo true || echo false)"

# 8. impact_preview
IMPACT_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 8:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if 'risk' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_impact_preview" "$([[ "$IMPACT_OK" == "yes" ]] && echo true || echo false)"

# 9. production_assist
PROD_OK=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 9:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if 'risk' in t and t.get('dryRun') == True else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "codelattice_production_assist" "$([[ "$PROD_OK" == "yes" ]] && echo true || echo false)"

# 10. cache_status (populated)
CACHE_POP=$(python3 -c "
import json
with open('$TMPFILE') as f:
    for line in f:
        line = line.strip()
        if not line: continue
        try:
            d = json.loads(line)
            if d.get('id') == 10:
                t = json.loads(d['result']['content'][0]['text'])
                print('yes' if t.get('entryCount', 0) >= 1 and 'maxEntries' in t else 'no')
                break
        except: pass
" 2>/dev/null || echo "no")
check_result "cache_status (populated)" "$([[ "$CACHE_POP" == "yes" ]] && echo true || echo false)"

rm -f "$TMPFILE"

echo ""
TOTAL=$((PASS + FAIL))
echo "Results: $PASS/$TOTAL passed, $FAIL failed"
if [[ "$FAIL" -gt 0 ]]; then
    echo "Some checks failed — see above for details."
    exit 1
fi
echo "All checks passed — MCP real client dry-run successful."
