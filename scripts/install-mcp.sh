#!/usr/bin/env bash
# install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.
#
# Usage: bash scripts/install-mcp.sh [options]
#
# Options:
#   --build           Build release binary with all language adapters (default)
#   --rust-only       Build release binary with Rust support only
#   --install-dir PATH Stable runtime directory for printed client config
#   --print-config    Print configuration snippets for AI clients
#   --dry-run         Show what would be done without doing it
#   --doctor          Run health checks on MCP setup
#   --help            Show help
#
# This script does NOT auto-write any client configuration files.
# It only prints copy-paste-ready config snippets.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$DEFAULT_REPO_ROOT}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
BIN_NAME="codelattice"
COMPAT_BIN_NAME="gitnexus-rust-core-cli"
WRAPPER="$REPO_ROOT/scripts/codelattice-mcp.sh"
DEFAULT_INSTALL_DIR="${HOME}/Desktop/CodeLattice-Tool"
INSTALL_DIR="${CODELATTICE_TOOL_DIR:-$DEFAULT_INSTALL_DIR}"

ACTION="build"
DRY_RUN=false
RUST_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build)        ACTION="build" ;;
        --rust-only)    RUST_ONLY=true ;;
        --install-dir)
            INSTALL_DIR="${2:-}"
            if [[ -z "$INSTALL_DIR" ]]; then
                echo "ERROR: --install-dir requires a path" >&2
                exit 1
            fi
            shift
            ;;
        --print-config) ACTION="print_config" ;;
        --dry-run)      DRY_RUN=true ;;
        --doctor)       ACTION="doctor" ;;
        --help|-h)
            cat <<'HELP'
install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.

Usage: bash scripts/install-mcp.sh [--build|--rust-only|--install-dir PATH|--print-config|--dry-run|--doctor|--help]

Options:
  --build           Build release binary with all language adapters (default)
  --rust-only       Build release binary with Rust support only (no optional adapters)
  --install-dir     Stable runtime directory for printed client config
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
    shift
done

echo "=== CodeLattice MCP Installer ==="
echo "Repo: $REPO_ROOT"
echo "Install dir: $INSTALL_DIR"
echo ""

# --- Build ---
if [[ "$ACTION" == "build" ]]; then
    if [[ "$RUST_ONLY" == "true" ]]; then
        echo "--- Building release binary (Rust only) ---"
        BUILD_CMD="cargo build --release -p gitnexus-rust-core-cli --bins"
    else
        echo "--- Building release binary (all language adapters) ---"
        BUILD_CMD="cargo build --release -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript --bins"
    fi

    if [[ "$DRY_RUN" == "true" ]]; then
        echo "  (dry-run) Would run: $BUILD_CMD"
        echo "  (dry-run) --manifest-path $REPO_ROOT/Cargo.toml"
    else
        $BUILD_CMD --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1
    fi

    BIN_PATH="$REPO_ROOT/target/release/$BIN_NAME"
    if [[ -x "$BIN_PATH" ]]; then
        VERSION=$("$BIN_PATH" --version 2>&1 || echo "unknown")
        echo ""
        echo "Build successful:"
        echo "  Binary:  $BIN_PATH"
        echo "  Version: $VERSION"
        if [[ "$RUST_ONLY" == "true" ]]; then
            echo "  Profile: rust-only"
        else
            echo "  Profile: all-languages"
        fi
    elif [[ "$DRY_RUN" != "true" ]]; then
        echo "WARNING: Binary not found after build: $BIN_PATH"
    fi

    echo ""
    echo "Next steps:"
    echo "  1. Test dev wrapper: bash $WRAPPER --self-test"
    echo "  2. Promote stable runtime: bash $REPO_ROOT/scripts/promote-to-local-tool.sh --install-dir \"$INSTALL_DIR\""
    echo "  3. Configure clients: bash $0 --install-dir \"$INSTALL_DIR\" --print-config"
fi

# --- Print Config ---
if [[ "$ACTION" == "print_config" ]]; then
    STABLE_WRAPPER="$INSTALL_DIR/codelattice-mcp.sh"
    WRAPPER_PATH="$STABLE_WRAPPER"
    BIN_PATH="$REPO_ROOT/target/release/$BIN_NAME"

    echo "--- Configuration Snippets ---"
    echo ""
    echo "IMPORTANT: Use the promoted stable wrapper path, not the binary directly."
    echo "Selected wrapper: $WRAPPER_PATH"
    if [[ -x "$STABLE_WRAPPER" ]]; then
        echo "Wrapper source:   stable runtime wrapper"
    else
        echo "Wrapper source:   stable runtime wrapper (not installed yet)"
        echo ""
        echo "Stable runtime not found. Run this first:"
        echo "  bash $REPO_ROOT/scripts/promote-to-local-tool.sh --install-dir \"$INSTALL_DIR\""
    fi
    echo ""
    echo "Development wrapper for contributor/debug only:"
    echo "  bash $WRAPPER"
    echo ""
    echo "Copy ONE of these into your AI client config file."
    echo "Do NOT add multiple entries for the same server."
    echo ""

    echo "=== Claude Desktop / Claude Code ==="
    echo "File: ~/Library/Application Support/Claude/claude_desktop_config.json"
    echo "  or: ~/.claude/claude_desktop_config.json"
    echo ""
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

    echo ""
    echo "=== Codex (OpenAI) ==="
    echo "File: ~/.codex/config.toml"
    echo ""
    cat <<TOML
[mcp_servers.codelattice]
type = "stdio"
command = "bash"
args = ["$WRAPPER_PATH"]
TOML

    echo ""
    echo "=== opencode ==="
    echo "File: ~/.config/opencode/opencode.json (global) or .opencode/config.json (project)"
    echo ""
    cat <<JSON
{
  "mcp": {
    "codelattice": {
      "type": "local",
      "command": ["$WRAPPER_PATH"],
      "enabled": true
    }
  }
}
JSON

    echo ""
    echo "=== With GitNexus (both side by side) ==="
    echo "If you also use GitNexus-RC MCP, add both as separate servers:"
    echo ""
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

    echo ""
    echo "Notes:"
    echo "  - CodeLattice MCP is a sidecar — it does NOT replace GitNexus-RC"
    echo "  - This script never writes client config; it only prints snippets"
    echo "  - Supports Rust, Cangjie, ArkTS, and TypeScript when built with --build"
    echo "  - 24 tools including process-local cache with mtime invalidation and prewarm"
    echo "  - Read-only — never modifies source code"
    echo "  - After config change, restart your AI client session to reload MCP tools"
fi

# --- Doctor ---
if [[ "$ACTION" == "doctor" ]]; then
    echo "=== CodeLattice MCP Doctor ==="
    echo ""
    PASS=0
    FAIL=0

    # 1. Check binary exists (prefer release)
    BIN_PATH=""
    BIN_PROFILE="unknown"
    for candidate in \
        "$REPO_ROOT/target/release/$BIN_NAME" \
        "$REPO_ROOT/target/debug/$BIN_NAME" \
        "$REPO_ROOT/target/release/$COMPAT_BIN_NAME" \
        "$REPO_ROOT/target/debug/$COMPAT_BIN_NAME"; do
        if [[ -x "$candidate" ]]; then
            BIN_PATH="$candidate"
            if [[ "$candidate" == *"/release/"* ]]; then
                BIN_PROFILE="release"
            else
                BIN_PROFILE="debug"
            fi
            break
        fi
    done

    if [[ -n "$BIN_PATH" ]]; then
        echo "PASS: binary found: $BIN_PATH ($BIN_PROFILE)"
        PASS=$((PASS + 1))
    else
        echo "FAIL: no binary found. Run: bash $0 --build"
        FAIL=$((FAIL + 1))
    fi

    # 2. Check wrapper
    if [[ -x "$WRAPPER" ]]; then
        echo "PASS: development wrapper script: $WRAPPER"
        PASS=$((PASS + 1))
    else
        echo "FAIL: development wrapper not found: $WRAPPER"
        FAIL=$((FAIL + 1))
    fi

    STABLE_WRAPPER="$INSTALL_DIR/codelattice-mcp.sh"
    if [[ -x "$STABLE_WRAPPER" ]]; then
        echo "PASS: stable runtime wrapper: $STABLE_WRAPPER"
        PASS=$((PASS + 1))
    else
        echo "WARN: stable runtime wrapper not found: $STABLE_WRAPPER"
        echo "      Run: bash $REPO_ROOT/scripts/promote-to-local-tool.sh --install-dir \"$INSTALL_DIR\""
    fi

    # 3-6: MCP checks (only if binary exists)
    if [[ -n "$BIN_PATH" ]]; then
        # 3. MCP handshake + profile detection
        INIT_RESP=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}' | "$BIN_PATH" mcp 2>/dev/null | head -1)
        if echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d['result']['serverInfo']['name']=='codelattice'" 2>/dev/null; then
            SERVER_VER=$(echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo']['version'])" 2>/dev/null || echo "unknown")
            CANGJIE_SUPPORT=$(echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('cangjieSupport','unknown'))" 2>/dev/null || echo "unknown")
            ARKTS_SUPPORT=$(echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('arktsSupport','unknown'))" 2>/dev/null || echo "unknown")
            TYPESCRIPT_SUPPORT=$(echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('typescriptSupport','unknown'))" 2>/dev/null || echo "unknown")
            TOOL_COUNT_INFO=$(echo "$INIT_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['result']['serverInfo'].get('toolCount','unknown'))" 2>/dev/null || echo "unknown")
            echo "PASS: MCP handshake (server v$SERVER_VER, cangjie=$CANGJIE_SUPPORT, arkts=$ARKTS_SUPPORT, typescript=$TYPESCRIPT_SUPPORT, tools=$TOOL_COUNT_INFO)"
            PASS=$((PASS + 1))
        else
            echo "FAIL: MCP handshake failed"
            FAIL=$((FAIL + 1))
            CANGJIE_SUPPORT="unknown"
            ARKTS_SUPPORT="unknown"
            TYPESCRIPT_SUPPORT="unknown"
        fi

        # 4. tools/list count
        TOOLS_RESP=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' | "$BIN_PATH" mcp 2>/dev/null | tail -1)
        TOOL_COUNT=$(echo "$TOOLS_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['result']['tools']))" 2>/dev/null || echo "0")
        if [[ "$TOOL_COUNT" -ge 21 ]]; then
            echo "PASS: tools/list returns $TOOL_COUNT tools"
            PASS=$((PASS + 1))
        else
            echo "FAIL: tools/list returned $TOOL_COUNT tools (expected >= 21)"
            FAIL=$((FAIL + 1))
        fi

        # 5. cache_status
        CACHE_RESP=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}\n' | "$BIN_PATH" mcp 2>/dev/null | tail -1)
        HAS_MAX=$(echo "$CACHE_RESP" | python3 -c "import json,sys; d=json.load(sys.stdin); t=d['result']['content'][0]['text']; j=json.loads(t); m=j.get('memory',{}); print('yes' if 'maxEntries' in m else 'no')" 2>/dev/null || echo "no")
        if [[ "$HAS_MAX" == "yes" ]]; then
            echo "PASS: cache_status has maxEntries"
            PASS=$((PASS + 1))
        else
            echo "FAIL: cache_status missing maxEntries"
            FAIL=$((FAIL + 1))
        fi

        # 6. Language adapter support check
        if [[ "$CANGJIE_SUPPORT" == "True" && "$ARKTS_SUPPORT" == "True" && "$TYPESCRIPT_SUPPORT" == "True" ]]; then
            echo "PASS: all optional language adapters compiled"
            PASS=$((PASS + 1))
        elif [[ "$CANGJIE_SUPPORT" == "False" || "$ARKTS_SUPPORT" == "False" || "$TYPESCRIPT_SUPPORT" == "False" ]]; then
            echo "FAIL: missing optional language adapter support"
            echo "      cangjie=$CANGJIE_SUPPORT arkts=$ARKTS_SUPPORT typescript=$TYPESCRIPT_SUPPORT"
            echo "      Fix: bash $0 --build"
            echo "      Or: cargo build --release -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript --bins"
            FAIL=$((FAIL + 1))
        else
            echo "WARN: language support could not be fully detected (cangjie=$CANGJIE_SUPPORT arkts=$ARKTS_SUPPORT typescript=$TYPESCRIPT_SUPPORT)"
        fi

        # 7. Cangjie smoke test (only if support is true)
        if [[ "$CANGJIE_SUPPORT" == "True" ]]; then
            CJGUI_PATH="$REPO_ROOT/fixtures/cangjie/portable-smoke"
            if [[ -d "$CJGUI_PATH" ]]; then
                CJ_SEARCH=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"doctor","version":"1.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_symbol_search","arguments":{"root":"%s","query":"init","language":"cangjie","limit":3}}}\n' "$CJGUI_PATH" | "$BIN_PATH" mcp 2>/dev/null)
                CJ_COUNT=$(echo "$CJ_SEARCH" | python3 -c "
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try:
        d = json.loads(line)
        if d.get('id') == 2:
            t = json.loads(d['result']['content'][0]['text'])
            print(t.get('matchCount', 0))
            break
    except: pass
" 2>/dev/null || echo "0")
                if [[ "$CJ_COUNT" -gt 0 ]]; then
                    echo "PASS: Cangjie symbol_search(init) returned $CJ_COUNT results"
                    PASS=$((PASS + 1))
                else
                    echo "FAIL: Cangjie symbol_search(init) returned 0 results"
                    FAIL=$((FAIL + 1))
                fi
            else
                echo "SKIP: Cangjie fixture smoke (fixture not found: $CJGUI_PATH)"
            fi
        fi
    else
        echo "SKIP: MCP checks (no binary)"
    fi

    echo ""
    echo "Results: $PASS passed, $FAIL failed"
    if [[ "$FAIL" -gt 0 ]]; then
        echo ""
        echo "Fix suggestions:"
        echo "  bash $0 --build              # Build all language adapters"
        echo "  bash $0 --build --rust-only  # Build Rust only (no optional adapters)"
        echo "  bash $WRAPPER --self-test"
        exit 1
    fi
    echo "All checks passed — MCP is ready for client integration."
fi
