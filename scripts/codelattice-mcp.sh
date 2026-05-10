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
    echo "codelattice-mcp-wrapper 0.2.0"
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

# --- Validate root ---
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
