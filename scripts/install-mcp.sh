#!/usr/bin/env bash
# install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.
#
# Usage: bash scripts/install-mcp.sh [options]
#
# Options:
#   --build           Build release binary (default action)
#   --print-config    Print configuration snippets for AI clients
#   --dry-run         Show what would be done without doing it
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
        --help|-h)
            cat <<'HELP'
install-mcp.sh — Build and configure CodeLattice MCP for local AI clients.

Usage: bash scripts/install-mcp.sh [--build|--print-config|--dry-run|--help]

Options:
  --build           Build release binary (default)
  --print-config    Print MCP client configuration snippets
  --dry-run         Show what would be done
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
    echo "  - 18 tools including process-local cache"
    echo "  - Read-only — never modifies source code"
fi
