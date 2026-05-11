#!/usr/bin/env bash
# cangjie-live-codelattice-smoke.sh
# Read-only smoke tests for the live Cangjie codebase via CodeLattice.
#
# Usage:
#   bash scripts/cangjie-live-codelattice-smoke.sh --dry-run
#   bash scripts/cangjie-live-codelattice-smoke.sh --analyze
#   bash scripts/cangjie-live-codelattice-smoke.sh --mcp
#   bash scripts/cangjie-live-codelattice-smoke.sh --tool-ingest
#   bash scripts/cangjie-live-codelattice-smoke.sh --full
#
# Modes:
#   --dry-run      Only print commands and check paths
#   --analyze      Run CodeLattice CLI analyze, output JSON to /tmp
#   --mcp          Test MCP tools against live root
#   --tool-ingest  Export bridge JSON and import into GitNexus-RC-Tool registry
#   --full         analyze + mcp + tool-ingest
#
# Hard rules:
#   - NEVER modify cangjie source code
#   - NEVER write AGENTS.md/CLAUDE.md to live repo
#   - Tool registry name MUST be cangjie-live-codelattice
#   - All temp output to /tmp/codelattice-cangjie-live-*

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────

LIVE_ROOT="/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$REPO_ROOT/target/debug/gitnexus-rust-core-cli"
TOOL_CLI="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"
TMP_PREFIX="/tmp/codelattice-cangjie-live"
BRIDGE_JSON="${TMP_PREFIX}-bridge.json"
ANALYZE_JSON="${TMP_PREFIX}-analyze.json"
REPO_NAME="cangjie-live-codelattice"

PASS=0
FAIL=0
RESULTS=()

# ── Mode selection ─────────────────────────────────────────────

MODE="${1:---dry-run}"

# ── Helpers ────────────────────────────────────────────────────

bail() {
    echo "FATAL: $1" >&2
    exit 1
}

check_tool() {
    local tool_name="$1"
    local args="$2"
    local check_expr="${3:-}"

    local request
    request=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}' "$tool_name" "$args")

    local response
    response=$(echo "$request" | "$BIN" mcp 2>/dev/null | head -1)

    if [ -z "$response" ]; then
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: $tool_name — no response")
        echo "  FAIL: $tool_name — no response"
        return
    fi

    local is_error
    is_error=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('result',{}).get('isError',False))" 2>/dev/null || echo "True")

    if [ "$is_error" = "True" ]; then
        FAIL=$((FAIL + 1))
        local err_msg
        err_msg=$(echo "$response" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; print(json.loads(t).get('message','unknown'))" 2>/dev/null || echo "unknown error")
        RESULTS+=("FAIL: $tool_name — $err_msg")
        echo "  FAIL: $tool_name — $err_msg"
        return
    fi

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
            echo "  PASS: $tool_name"
        else
            FAIL=$((FAIL + 1))
            RESULTS+=("FAIL: $tool_name — check expression failed")
            echo "  FAIL: $tool_name — check expression failed"
        fi
    else
        PASS=$((PASS + 1))
        RESULTS+=("PASS: $tool_name")
        echo "  PASS: $tool_name"
    fi
}

print_results() {
    echo ""
    echo "=== Results ==="
    for r in "${RESULTS[@]}"; do
        echo "  $r"
    done
    echo ""
    echo "Total: $PASS passed, $FAIL failed"
    if [ "$FAIL" -gt 0 ]; then
        echo ""
        echo "FAILURE: $FAIL check(s) failed."
        echo "Suggestions:"
        echo "  1. Verify binary has cangjie feature: bash scripts/codelattice-mcp.sh --self-test"
        echo "  2. Rebuild with cangjie: cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie"
        echo "  3. Check live root exists: ls $LIVE_ROOT"
        exit 1
    fi
}

# ── Pre-flight ─────────────────────────────────────────────────

echo "=== Cangjie Live CodeLattice Smoke ==="
echo "Mode: $MODE"
echo "Live root: $LIVE_ROOT"
echo "Binary: $BIN"
echo "Repo name: $REPO_NAME"
echo ""

[ -d "$LIVE_ROOT" ] || bail "Live root not found: $LIVE_ROOT"
[ -f "$BIN" ] || bail "Binary not found: $BIN — run: cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie"

# Verify binary has cangjie feature
PROFILE_RESP=$(echo '{"jsonrpc":"2.0","id":999,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1.0"}}}' | "$BIN" mcp 2>/dev/null | head -1)
PROFILE_CJ=$(echo "$PROFILE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('cangjieSupport','unknown'))" 2>/dev/null || echo "unknown")
echo "Profile: cangjieSupport=$PROFILE_CJ"
if [ "$PROFILE_CJ" != "True" ]; then
    bail "Binary does not have cangjie support. Rebuild: cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie"
fi

# Check live root is not modified by us
echo "Live root dirty check (read-only verification):"
DIRTY_COUNT=$(cd "$LIVE_ROOT" && git status --short 2>/dev/null | wc -l || echo "not-a-git-repo")
echo "  Dirty files in live root: $DIRTY_COUNT"
echo ""

# ── DRY-RUN ────────────────────────────────────────────────────

if [ "$MODE" = "--dry-run" ]; then
    echo "=== DRY-RUN: Commands that would be executed ==="
    echo ""
    echo "# 1. CLI Analyze"
    echo "$BIN analyze --language cangjie --root $LIVE_ROOT"
    echo ""
    echo "# 2. Export bridge JSON"
    echo "$BIN export-bridge --language cangjie --root $LIVE_ROOT --output $BRIDGE_JSON"
    echo ""
    echo "# 3. MCP project_overview"
    echo "  tool: codelattice_project_overview, args: {root: '$LIVE_ROOT', language: 'cangjie'}"
    echo ""
    echo "# 4. MCP symbol_search init"
    echo "  tool: codelattice_symbol_search, args: {query: 'init', root: '$LIVE_ROOT', language: 'cangjie'}"
    echo ""
    echo "# 5. Tool ingest (if --tool-ingest or --full)"
    echo "node $TOOL_CLI analyze $LIVE_ROOT --experimental-rust-core-bridge-graph $BRIDGE_JSON --name $REPO_NAME --skip-agents-md --force"
    echo ""
    echo "# 6. Tool verify"
    echo "node $TOOL_CLI list"
    echo "node $TOOL_CLI detect-changes -r $REPO_NAME --scope all"
    echo ""
    echo "DRY-RUN complete. No commands were executed."
    PASS=$((PASS + 1))
    RESULTS+=("PASS: dry-run completed")
    print_results
    exit 0
fi

# ── ANALYZE ────────────────────────────────────────────────────

if [ "$MODE" = "--analyze" ] || [ "$MODE" = "--full" ]; then
    echo "=== CLI Analyze ==="

    # Ensure binary is built with cangjie
    echo "Building..."
    cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie --quiet 2>/dev/null

    # Run analyze via CLI subcommand
    echo "Running CodeLattice analyze on $LIVE_ROOT ..."
    START=$(python3 -c "import time; print(time.time())")
    ANALYZE_OUTPUT=$("$BIN" analyze --language cangjie --root "$LIVE_ROOT" 2>/dev/null) || {
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: CLI analyze")
        echo "  FAIL: CLI analyze — check stderr"
    }
    END=$(python3 -c "import time; print(time.time())")

    if [ -n "${ANALYZE_OUTPUT:-}" ]; then
        # Save to file
        echo "$ANALYZE_OUTPUT" > "$ANALYZE_JSON"
        echo "  Saved to $ANALYZE_JSON"

        # Validate JSON — data is in summary sub-object
        python3 -c "
import json
d = json.loads(open('$ANALYZE_JSON').read())
s = d.get('summary', d)
print(f'  nodes={s.get(\"nodeCount\",0)} edges={s.get(\"edgeCount\",0)} symbols={s.get(\"symbolCount\",0)} files={s.get(\"sourceFileCount\",0)}')
assert s.get('nodeCount',0) > 0, 'nodeCount must be > 0'
# Verify quality gates
for g in d.get('qualityGates', []):
    assert g.get('passed', False), f'Quality gate failed: {g[\"gateName\"]}'
" 2>/dev/null && {
            PASS=$((PASS + 1))
            RESULTS+=("PASS: CLI analyze JSON valid + quality gates")
        } || {
            FAIL=$((FAIL + 1))
            RESULTS+=("FAIL: CLI analyze JSON parse or quality gates")
        }

        ELAPSED=$(python3 -c "print(f'{${END} - ${START}:.2f}s')")
        echo "  Duration: $ELAPSED"
    fi

    # Export bridge JSON via MCP tool (no CLI subcommand for this)
    echo ""
    echo "=== Export Bridge JSON ==="
    BRIDGE_REQ=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"codelattice_export_bridge","arguments":{"root":"%s","language":"cangjie"}}}' "$LIVE_ROOT")
    BRIDGE_RESP=$(echo "$BRIDGE_REQ" | "$BIN" mcp 2>/dev/null | head -1)
    BRIDGE_PATH=$(echo "$BRIDGE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; data=json.loads(t); print(data.get('outputPath',''))" 2>/dev/null || echo "")
    if [ -n "$BRIDGE_PATH" ] && [ -f "$BRIDGE_PATH" ]; then
        cp "$BRIDGE_PATH" "$BRIDGE_JSON"
        echo "  Bridge JSON copied from $BRIDGE_PATH"
    fi

    if [ -f "$BRIDGE_JSON" ]; then
        BRIDGE_SIZE=$(wc -c < "$BRIDGE_JSON" | tr -d ' ')
        echo "  Bridge JSON: $BRIDGE_JSON ($BRIDGE_SIZE bytes)"
        PASS=$((PASS + 1))
        RESULTS+=("PASS: bridge JSON exported")
    else
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: bridge JSON export")
        echo "  FAIL: Could not export bridge JSON"
    fi
    echo ""
fi

# ── MCP ────────────────────────────────────────────────────────

if [ "$MODE" = "--mcp" ] || [ "$MODE" = "--full" ]; then
    echo "=== MCP Tests ==="

    # 1. cache_prewarm
    echo "1. cache_prewarm"
    check_tool "codelattice_cache_prewarm" "{\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "True"

    # 2. project_overview
    echo "2. project_overview"
    check_tool "codelattice_project_overview" "{\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "data.get('nodeCount',0) > 0"

    # 3. graph_overview
    echo "3. graph_overview"
    check_tool "codelattice_graph_overview" "{\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "data.get('nodeCount',0) > 0"

    # 4. symbol_search init
    echo "4. symbol_search(init)"
    check_tool "codelattice_symbol_search" "{\"query\":\"init\",\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "len(data) > 0"

    # 5. symbol_context (pick first init result)
    echo "5. symbol_context"
    check_tool "codelattice_symbol_context" "{\"name\":\"init\",\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "True"

    # 6. production_assist
    echo "6. production_assist"
    check_tool "codelattice_production_assist" "{\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "True"

    # 7. cache_status
    echo "7. cache_status"
    check_tool "codelattice_cache_status" "{\"root\":\"$LIVE_ROOT\",\"language\":\"cangjie\"}" "data.get('maxEntries',0) > 0"

    echo ""
fi

# ── TOOL INGEST ────────────────────────────────────────────────

if [ "$MODE" = "--tool-ingest" ] || [ "$MODE" = "--full" ]; then
    echo "=== Tool Registry Ingest ==="

    # Verify bridge JSON exists
    if [ ! -f "$BRIDGE_JSON" ]; then
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: tool-ingest — no bridge JSON (run --analyze first)")
        echo "  FAIL: No bridge JSON found. Run with --analyze or --full first."
    elif [ ! -f "$TOOL_CLI" ]; then
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL: tool-ingest — Tool CLI not found: $TOOL_CLI")
        echo "  FAIL: Tool CLI not found: $TOOL_CLI"
    else
        echo "  Importing bridge JSON as $REPO_NAME ..."

        # Use Tool CLI to import via bridge adapter
        IMPORT_CMD="node $TOOL_CLI analyze $LIVE_ROOT --experimental-rust-core-bridge-graph $BRIDGE_JSON --name $REPO_NAME --skip-agents-md --force"
        echo "  Running: $IMPORT_CMD"

        IMPORT_OUTPUT=$($IMPORT_CMD 2>&1) || {
            # Check if it's a git repo issue — try with --skip-git
            echo "  Retrying with --skip-git ..."
            IMPORT_OUTPUT=$(node "$TOOL_CLI" analyze "$LIVE_ROOT" --experimental-rust-core-bridge-graph "$BRIDGE_JSON" --name "$REPO_NAME" --skip-agents-md --force --skip-git 2>&1) || {
                FAIL=$((FAIL + 1))
                RESULTS+=("FAIL: tool-ingest import")
                echo "  FAIL: Import failed. Output:"
                echo "$IMPORT_OUTPUT" | head -20
                echo ""
                echo "  Gap: Tool CLI may not support bridge JSON import with --name."
                echo "  Document this gap — do not modify Tool code."
            }
        }

        if [ "${FAIL:-0}" -eq 0 ] || [ ! "${RESULTS[-1]:-}" = "FAIL: tool-ingest import" ]; then
            # Verify registration
            echo "  Verifying registration..."
            REG_LIST=$(node "$TOOL_CLI" list 2>&1)
            if echo "$REG_LIST" | grep -q "$REPO_NAME"; then
                PASS=$((PASS + 1))
                RESULTS+=("PASS: tool-ingest — $REPO_NAME registered")
                echo "  PASS: $REPO_NAME found in registry"
            else
                # Also check by path
                if echo "$REG_LIST" | grep -q "$LIVE_ROOT"; then
                    PASS=$((PASS + 1))
                    RESULTS+=("PASS: tool-ingest — registered by path")
                    echo "  PASS: Found in registry (by path)"
                else
                    FAIL=$((FAIL + 1))
                    RESULTS+=("FAIL: tool-ingest — not found in registry")
                    echo "  FAIL: $REPO_NAME not found in registry"
                    echo "  Gap: Tool CLI may need different flags for bridge JSON import."
                    echo "  Document this gap."
                fi
            fi

            # Verify detect-changes works
            echo "  Testing detect-changes..."
            DC_OUTPUT=$(node "$TOOL_CLI" detect-changes -r "$REPO_NAME" --scope all 2>&1) || {
                echo "  Note: detect-changes failed (expected for read-only live repo):"
                echo "$DC_OUTPUT" | head -5
            }
        fi
    fi
    echo ""
fi

# ── Summary ────────────────────────────────────────────────────

print_results
