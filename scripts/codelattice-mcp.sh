#!/usr/bin/env bash
# codelattice-mcp.sh — Stable MCP startup wrapper for AI sidecar integration.
#
# Usage: bash scripts/codelattice-mcp.sh [--help|--version]
#
# This wrapper provides a single entry point for MCP clients (Codex, Claude,
# opencode, etc.) to start the CodeLattice MCP server without knowing the
# internal cargo command structure.
#
# Environment variables:
#   CODELATTICE_ROOT       Override CodeLattice source root (default: auto-detect)
#   CODELATTICE_MCP_BIN    Use a pre-built binary instead of cargo run
#   CODELATTICE_LOG_LEVEL  Log verbosity (reserved; server does not read this yet)
#
# Binary selection order:
#   1. CODELATTICE_MCP_BIN if set and executable
#   2. target/release/gitnexus-rust-core-cli if exists and executable
#   3. target/debug/gitnexus-rust-core-cli if exists and executable
#   4. cargo run -p gitnexus-rust-core-cli -- mcp (builds on first call)
#
# The MCP server speaks newline-delimited JSON-RPC over stdio.
# Logging goes to stderr only — stdout is pure JSON-RPC.

set -euo pipefail

# --- Auto-detect root from script location ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CODELATTICE_ROOT="${CODELATTICE_ROOT:-$DEFAULT_ROOT}"
CODELATTICE_MCP_BIN="${CODELATTICE_MCP_BIN:-}"
CODELATTICE_LOG_LEVEL="${CODELATTICE_LOG_LEVEL:-}"

# --- CLI flags ---
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<'EOF'
codelattice-mcp.sh — CodeLattice MCP Server Startup Wrapper

Starts the CodeLattice MCP server for AI sidecar integration.
The server speaks newline-delimited JSON-RPC over stdio.

Usage:
  bash scripts/codelattice-mcp.sh [options]

Options:
  --help, -h     Show this help message
  --version      Print version info

Environment:
  CODELATTICE_ROOT       Source root (auto-detected from script location)
  CODELATTICE_MCP_BIN    Path to pre-built binary (skips cargo run)
  CODELATTICE_LOG_LEVEL  Reserved for future log control

Binary selection:
  1. CODELATTICE_MCP_BIN (if set)
  2. target/release/gitnexus-rust-core-cli (if exists)
  3. target/debug/gitnexus-rust-core-cli (if exists)
  4. cargo run (fallback, builds if needed)

Examples:
  # Default startup (from any cwd)
  bash /path/to/codelattice/scripts/codelattice-mcp.sh

  # With pre-built binary
  CODELATTICE_MCP_BIN=/usr/local/bin/gitnexus-rust-core-cli \
      bash scripts/codelattice-mcp.sh

  # In MCP client config (Codex / Claude / opencode)
  # See docs/architecture/mcp-local-client-setup.md
EOF
    exit 0
fi

if [[ "${1:-}" == "--version" ]]; then
    echo "codelattice-mcp-wrapper 0.5.0"
    echo "  root: $CODELATTICE_ROOT"
    # Try to get binary version
    if [[ -n "$CODELATTICE_MCP_BIN" && -x "$CODELATTICE_MCP_BIN" ]]; then
        echo "  bin:  $CODELATTICE_MCP_BIN"
    elif [[ -x "$CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli" ]]; then
        echo "  bin:  $CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli"
    elif [[ -x "$CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli" ]]; then
        echo "  bin:  $CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli"
    else
        echo "  bin:  (cargo run fallback)"
    fi
    exit 0
fi

if [[ "${1:-}" == "--self-test" ]]; then
    echo "codelattice-mcp self-test"
    echo "  root: $CODELATTICE_ROOT"

    # Check root validity
    if [[ ! -d "$CODELATTICE_ROOT" ]]; then
        echo "FAIL: CODELATTICE_ROOT does not exist: $CODELATTICE_ROOT"
        exit 1
    fi
    if [[ ! -f "$CODELATTICE_ROOT/Cargo.toml" ]]; then
        echo "FAIL: No Cargo.toml in CODELATTICE_ROOT"
        exit 1
    fi
    echo "  root: OK"

    # Check binary
    SELF_TEST_BIN=""
    if [[ -n "$CODELATTICE_MCP_BIN" && -x "$CODELATTICE_MCP_BIN" ]]; then
        SELF_TEST_BIN="$CODELATTICE_MCP_BIN"
    elif [[ -x "$CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli" ]]; then
        SELF_TEST_BIN="$CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli"
    elif [[ -x "$CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli" ]]; then
        SELF_TEST_BIN="$CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli"
    fi

    if [[ -n "$SELF_TEST_BIN" ]]; then
        echo "  bin:  $SELF_TEST_BIN"
        VER=$("$SELF_TEST_BIN" --version 2>&1 || echo "unknown")
        echo "  ver:  $VER"
    else
        echo "  bin:  (cargo run fallback — no pre-built binary found)"
        echo "  hint: Run 'cargo build -p gitnexus-rust-core-cli' first, or set CODELATTICE_MCP_BIN"
    fi

    # Quick MCP handshake test
    if [[ -n "$SELF_TEST_BIN" ]]; then
        echo ""
        echo "  MCP handshake test..."
        RESP=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"self-test","version":"1.0"}}}' | "$SELF_TEST_BIN" mcp 2>/dev/null | head -1)
        if echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['result']['serverInfo']['name']=='codelattice'" 2>/dev/null; then
            VER=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo']['version'])" 2>/dev/null || echo "unknown")
            echo "  MCP:  OK (server v$VER)"
        else
            echo "  MCP:  FAIL — unexpected response"
            exit 1
        fi

        # Extended test: tools/list + cache_status
        echo ""
        echo "  Extended checks..."
        MULTI_RESP=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"self-test","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}\n' | "$SELF_TEST_BIN" mcp 2>/dev/null)

        TOOL_COUNT=$(echo "$MULTI_RESP" | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try:
        d = json.loads(line)
        if d.get('id') == 2:
            print(len(d['result']['tools']))
            break
    except: pass
" 2>/dev/null || echo "0")
        if [[ "$TOOL_COUNT" -ge 20 ]]; then
            echo "  tools/list: OK ($TOOL_COUNT tools)"
        else
            echo "  tools/list: FAIL ($TOOL_COUNT tools, expected >= 20)"
        fi

        CACHE_OK=$(echo "$MULTI_RESP" | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try:
        d = json.loads(line)
        if d.get('id') == 3:
            t = json.loads(d['result']['content'][0]['text'])
            print('yes' if 'maxEntries' in t and 'totalEvictions' in t else 'no')
            break
    except: pass
" 2>/dev/null || echo "no")
        if [[ "$CACHE_OK" == "yes" ]]; then
            echo "  cache_status: OK (has maxEntries, totalEvictions)"
        else
            echo "  cache_status: FAIL (missing fields)"
        fi
    fi

    echo ""
    echo "Self-test passed."
    exit 0
fi
if [[ ! -d "$CODELATTICE_ROOT" ]]; then
    echo "ERROR: CODELATTICE_ROOT does not exist: $CODELATTICE_ROOT" >&2
    exit 1
fi

if [[ ! -f "$CODELATTICE_ROOT/Cargo.toml" ]]; then
    echo "ERROR: No Cargo.toml in CODELATTICE_ROOT: $CODELATTICE_ROOT" >&2
    exit 1
fi

# --- Select binary ---
BIN=""
if [[ -n "$CODELATTICE_MCP_BIN" ]]; then
    if [[ ! -x "$CODELATTICE_MCP_BIN" ]]; then
        echo "ERROR: CODELATTICE_MCP_BIN not executable: $CODELATTICE_MCP_BIN" >&2
        exit 1
    fi
    BIN="$CODELATTICE_MCP_BIN"
elif [[ -x "$CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli" ]]; then
    BIN="$CODELATTICE_ROOT/target/release/gitnexus-rust-core-cli"
elif [[ -x "$CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli" ]]; then
    BIN="$CODELATTICE_ROOT/target/debug/gitnexus-rust-core-cli"
fi

# --- Launch ---
if [[ -n "$BIN" ]]; then
    exec "$BIN" mcp
else
    exec cargo run --manifest-path "$CODELATTICE_ROOT/Cargo.toml" -p gitnexus-rust-core-cli --quiet -- mcp
fi
