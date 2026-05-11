#!/usr/bin/env bash
# install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.
#
# Usage: bash scripts/install-mcp.sh [options]
#
# Options:
#   --build           Build release binary (default action)
#   --print-config    Print configuration snippets for AI clients
#   --dry-run         Show what would be done without doing it
#   --doctor          Run health checks on MCP setup
#   --help            Show help
#
# This script does NOT auto-write any client configuration files.
# It only prints copy-paste-ready config snippets.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_NAME="gitnexus-rust-core-cli"
WRAPPER="$SCRIPT_DIR/codelattice-mcp.sh"

ACTION="build"
DRY_RUN=false

for arg in "$@"; do
    case "$arg" in
        --build)        ACTION="build" ;;
        --print-config) ACTION="print_config" ;;
        --dry-run)      DRY_RUN=true ;;
        --doctor)       ACTION="doctor" ;;
        --help|-h)
            cat <<'HELP'
install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.

Usage: bash scripts/install-mcp.sh [--build|--print-config|--dry-run|--doctor|--help]

Options:
  --build           Build release binary (default)
  --print-config    Print MCP client configuration snippets
  --dry-run         Show what would be done
  --doctor          Run health checks on the MCP setup
  --help, -h        Show this help

This script does NOT modify any client configuration files automatically.
It prints ready-to-copy config snippets that you paste into your AI client.
HELP
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Use --help for usage."
            exit 1
            ;;
    esac
done

echo "=== CodeLattice MCP Installer ==="
echo "Repo: $REPO_ROOT"
echo ""

# --- Build ---
if [[ "$ACTION" == "build" ]]; then
    echo "--- Building release binary ---"
    if [[ "$DRY_RUN" == "true" ]]; then
        echo "  (dry-run) Would run: cargo build --release -p gitnexus-rust-core-cli"
    else
        cargo build --release -p gitnexus-rust-core-cli --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1
    fi

    BIN_PATH="$REPO_ROOT/target/release/$BIN_NAME"
    if [[ -x "$BIN_PATH" ]]; then
        VERSION=$("$BIN_PATH" --version 2>&1 || echo "unknown")
        echo ""
        echo "Build successful:"
        echo "  Binary:  $BIN_PATH"
        echo "  Version: $VERSION"
    elif [[ "$DRY_RUN" != "true" ]]; then
        echo "WARNING: Binary not found after build: $BIN_PATH"
    fi

    echo ""
    echo "Next steps:"
    echo "  1. Test:     $WRAPPER --self-test"
    echo "  2. Configure: bash $0 --print-config"
fi

# --- Print Config ---
if [[ "$ACTION" == "print_config" ]]; then
    WRAPPER_PATH="$WRAPPER"
    BIN_PATH="$REPO_ROOT/target/release/$BIN_NAME"

    echo "--- Configuration Snippets ---"
    echo ""
    echo "Copy ONE of these into your AI client config file."
    echo "Do NOT add multiple entries for the same server."
    echo ""

    echo "=== Claude Desktop / Claude Code ==="
    echo "File: ~/Library/Application Support/Claude/claude_desktop_config.json"
    echo "  or: ~/.claude/claude_desktop_config.json"
    echo ""
    if [[ -x "$BIN_PATH" ]]; then
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "$BIN_PATH",
      "args": ["mcp"]
    }
  }
}
JSON
    else
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["$WRAPPER_PATH"]
    }
  }
}
JSON
    fi

    echo ""
    echo "=== Codex (OpenAI) ==="
    echo "File: ~/.codex/config.toml"
    echo ""
    if [[ -x "$BIN_PATH" ]]; then
        cat <<TOML
[mcp_servers.codelattice]
command = "$BIN_PATH"
args = ["mcp"]
TOML
    else
        cat <<TOML
[mcp_servers.codelattice]
command = "bash"
args = ["$WRAPPER_PATH"]
TOML
    fi

    echo ""
    echo "=== opencode ==="
    echo "File: .opencode/config.json (project-level) or ~/.opencode/config.json"
    echo ""
    if [[ -x "$BIN_PATH" ]]; then
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "$BIN_PATH",
      "args": ["mcp"]
    }
  }
}
JSON
    else
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["$WRAPPER_PATH"]
    }
  }
}
JSON
    fi

    echo ""
    echo "=== With GitNexus (both side by side) ==="
    echo "If you also use GitNexus-RC MCP, add both as separate servers:"
    echo ""
    if [[ -x "$BIN_PATH" ]]; then
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "$BIN_PATH",
      "args": ["mcp"]
    },
    "gitnexus": {
      "command": "node",
      "args": ["/path/to/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"]
    }
  }
}
JSON
    else
        cat <<JSON
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["$WRAPPER_PATH"]
    },
    "gitnexus": {
      "command": "node",
      "args": ["/path/to/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"]
    }
  }
}
JSON
    fi

    echo ""
    echo "Notes:"
    echo "  - CodeLattice MCP is a sidecar — it does NOT replace GitNexus-RC"
    echo "  - Supports Rust and Cangjie analysis only"
    echo "  - 20 tools including process-local cache with mtime invalidation"
    echo "  - Read-only — never modifies source code"
fi

# --- Doctor ---
if [[ "$ACTION" == "doctor" ]]; then
    echo "=== CodeLattice MCP Doctor ==="
    echo ""
    PASS=0
    FAIL=0

    # 1. Check binary exists
    BIN_PATH=""
    for candidate in \
        "$REPO_ROOT/target/release/$BIN_NAME" \
        "$REPO_ROOT/target/debug/$BIN_NAME"; do
        if [[ -x "$candidate" ]]; then
            BIN_PATH="$candidate"
            break
        fi
    done

    if [[ -n "$BIN_PATH" ]]; then
        echo "PASS: binary found: $BIN_PATH"
        PASS=$((PASS + 1))
    else
        echo "FAIL: no binary found. Run: cargo build -p gitnexus-rust-core-cli"
        FAIL=$((FAIL + 1))
    fi

    # 2. Check wrapper
    if [[ -x "$WRAPPER" ]]; then
        echo "PASS: wrapper script: $WRAPPER"
        PASS=$((PASS + 1))
    else
        echo "FAIL: wrapper not found: $WRAPPER"
        FAIL=$((FAIL + 1))
    fi

    # 3. MCP handshake
    if [[ -n "$BIN_PATH" ]]; then
        RESP=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}' | "$BIN_PATH" mcp 2>/dev/null | head -1)
        if echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['result']['serverInfo']['name']=='codelattice'" 2>/dev/null; then
            SERVER_VER=$(echo "$RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo']['version'])" 2>/dev/null || echo "unknown")
            echo "PASS: MCP handshake (server v$SERVER_VER)"
            PASS=$((PASS + 1))
        else
            echo "FAIL: MCP handshake failed"
            FAIL=$((FAIL + 1))
        fi

        # 4. tools/list
        TOOLS_RESP=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' | "$BIN_PATH" mcp 2>/dev/null | tail -1)
        TOOL_COUNT=$(echo "$TOOLS_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))" 2>/dev/null || echo "0")
        if [[ "$TOOL_COUNT" -ge 20 ]]; then
            echo "PASS: tools/list returns $TOOL_COUNT tools"
            PASS=$((PASS + 1))
        else
            echo "FAIL: tools/list returned $TOOL_COUNT tools (expected >= 20)"
            FAIL=$((FAIL + 1))
        fi

        # 5. cache_status
        CACHE_RESP=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}\n' | "$BIN_PATH" mcp 2>/dev/null | tail -1)
        HAS_MAX=$(echo "$CACHE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; j=json.loads(t); print('yes' if 'maxEntries' in j else 'no')" 2>/dev/null || echo "no")
        if [[ "$HAS_MAX" == "yes" ]]; then
            echo "PASS: cache_status has maxEntries"
            PASS=$((PASS + 1))
        else
            echo "FAIL: cache_status missing maxEntries"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "SKIP: MCP checks (no binary)"
    fi

    echo ""
    echo "Results: $PASS passed, $FAIL failed"
    if [[ "$FAIL" -gt 0 ]]; then
        echo ""
        echo "Fix suggestions:"
        echo "  cargo build -p gitnexus-rust-core-cli"
        echo "  bash $WRAPPER --self-test"
        exit 1
    fi
    echo "All checks passed — MCP is ready for client integration."
fi
