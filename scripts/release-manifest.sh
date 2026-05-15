#!/usr/bin/env bash
# release-manifest.sh — Generate a release manifest JSON from the current checkout.
#
# Usage: bash scripts/release-manifest.sh [--binary <path>] [--output <path>]
#
# The manifest captures version, build info, and MCP profile data.
# It does NOT include local machine paths in the output.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

BINARY=""
OUTPUT=""
SKIP_BUILD_CHECK=false

usage() {
    cat <<'HELP'
release-manifest.sh — Generate a release manifest JSON.

Usage:
  bash scripts/release-manifest.sh [options]

Options:
  --binary <path>       Path to codelattice binary (default: target/release/codelattice)
  --output <path>       Output file path (default: stdout)
  --skip-build-check    Skip binary existence check
  --help, -h            Show this help

The manifest includes version, commit, platform, features, checksums, and MCP profile.
No local machine paths are included in the output.
HELP
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)
            BINARY="${2:-}"
            if [[ -z "$BINARY" ]]; then
                echo "ERROR: --binary requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --output)
            OUTPUT="${2:-}"
            if [[ -z "$OUTPUT" ]]; then
                echo "ERROR: --output requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --skip-build-check)
            SKIP_BUILD_CHECK=true
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

# --- Version from Cargo.toml ---
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -1)"
if [[ -z "$VERSION" ]]; then
    echo "ERROR: cannot read version from Cargo.toml" >&2
    exit 1
fi

# --- Binary detection ---
if [[ -z "$BINARY" ]]; then
    if [[ -x "$REPO_ROOT/target/release/codelattice" ]]; then
        BINARY="$REPO_ROOT/target/release/codelattice"
    elif [[ -x "$REPO_ROOT/target/debug/codelattice" ]]; then
        BINARY="$REPO_ROOT/target/debug/codelattice"
    else
        if [[ "$SKIP_BUILD_CHECK" == "true" ]]; then
            BINARY=""
        else
            echo "ERROR: no codelattice binary found. Run: cargo build --release" >&2
            exit 1
        fi
    fi
fi

# --- Git info ---
SOURCE_COMMIT="unknown"
SOURCE_REMOTE="unknown"
if git -C "$REPO_ROOT" rev-parse HEAD &>/dev/null; then
    SOURCE_COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD)"
    SOURCE_REMOTE="$(git -C "$REPO_ROOT" remote get-url gitcode 2>/dev/null || git -C "$REPO_ROOT" remote get-url origin 2>/dev/null || echo "unknown")"
fi

# --- Platform ---
PLATFORM="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"

# --- Build tools ---
RUSTC_VERSION="$(rustc --version 2>/dev/null || echo "unknown")"
CARGO_VERSION="$(cargo --version 2>/dev/null || echo "unknown")"

# --- Enabled features from default + optional ---
FEATURES_DEFAULT="tree-sitter-extraction"
FEATURES_ENABLED="$FEATURES_DEFAULT"
if [[ -n "$BINARY" ]]; then
    # Check if cangjie support is available by probing MCP
    _PROFILE="$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"manifest","version":"1.0"}}}' | "$BINARY" mcp 2>/dev/null | head -1 || true)"
    MCP_VERSION="$(echo "$_PROFILE" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("version","unknown"))' 2>/dev/null || echo "unknown")"
    CANGJIE_SUPPORT="$(echo "$_PROFILE" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cangjieSupport",False)).lower())' 2>/dev/null || echo "false")"
    TOOL_COUNT="$(echo "$_PROFILE" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("toolCount",0))' 2>/dev/null || echo "0")"
    BINARY_SHA256="$(shasum -a 256 "$BINARY" | awk '{print $1}')"
else
    MCP_VERSION="unknown"
    CANGJIE_SUPPORT="unknown"
    TOOL_COUNT="unknown"
    BINARY_SHA256="unknown"
fi

# --- Build manifest JSON ---
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

MANIFEST=$(cat <<JSON
{
  "name": "CodeLattice",
  "releaseVersion": "$VERSION",
  "sourceCommit": "$SOURCE_COMMIT",
  "sourceRemote": "$SOURCE_REMOTE",
  "platform": "$PLATFORM",
  "buildProfile": "release",
  "builtAt": "$TIMESTAMP",
  "rustcVersion": "$RUSTC_VERSION",
  "cargoVersion": "$CARGO_VERSION",
  "enabledFeatures": "$FEATURES_ENABLED",
  "binaryName": "codelattice",
  "binarySha256": "$BINARY_SHA256",
  "mcpProfile": {
    "serverVersion": "$MCP_VERSION",
    "toolCount": $TOOL_COUNT,
    "cangjieSupport": $CANGJIE_SUPPORT
  },
  "supportedLanguages": {
    "rust": "stable",
    "cangjie": "stable",
    "arkts": "production-trial",
    "typescript": "phase-a"
  },
  "knownLimitationsDoc": "CHANGELOG.md",
  "releaseStatus": "external-beta"
}
JSON
)

if [[ -n "$OUTPUT" ]]; then
    echo "$MANIFEST" > "$OUTPUT"
    echo "Manifest written to: $OUTPUT"
else
    echo "$MANIFEST"
fi
