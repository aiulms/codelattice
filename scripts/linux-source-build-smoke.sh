#!/usr/bin/env bash
# linux-source-build-smoke.sh — Source-build compatibility preflight for Linux/openEuler.
#
# This script is intentionally usable on non-Linux developer machines too. It
# reports the current platform, builds into a temporary CARGO_TARGET_DIR, and
# verifies the CLI + MCP path without writing AI client configuration files.

set -euo pipefail

usage() {
    cat <<'HELP'
linux-source-build-smoke.sh — Verify the source-build path for Linux/openEuler.

Usage:
  bash scripts/linux-source-build-smoke.sh [options]

Options:
  --features <list>          Cargo feature list to build/test with
  --all-language-features    Use all optional language adapters
  --target-dir <path>        Cargo target dir (default: temp dir)
  --skip-fmt                 Skip cargo fmt --check
  --skip-tests               Skip cargo test --test mcp_server
  --keep-temp                Keep the auto-created target dir
  --help, -h                 Show this help

The script does not run npm/tsc/project scripts and does not modify AI client
configuration files.
HELP
}

FEATURES=""
SKIP_FMT=false
SKIP_TESTS=false
KEEP_TEMP=false
CUSTOM_TARGET_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --features)
            FEATURES="${2:-}"
            if [[ -z "$FEATURES" ]]; then
                echo "ERROR: --features requires a comma-separated feature list" >&2
                exit 1
            fi
            shift 2
            ;;
        --all-language-features)
            FEATURES="tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python"
            shift
            ;;
        --target-dir)
            CUSTOM_TARGET_DIR="${2:-}"
            if [[ -z "$CUSTOM_TARGET_DIR" ]]; then
                echo "ERROR: --target-dir requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --skip-fmt)
            SKIP_FMT=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --keep-temp)
            KEEP_TEMP=true
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

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

if [[ ! -f "$REPO_ROOT/Cargo.toml" ]]; then
    echo "ERROR: CODELATTICE_ROOT does not look like a CodeLattice checkout: $REPO_ROOT" >&2
    exit 1
fi

TMP_ROOT="${TMPDIR:-/tmp}"
AUTO_TARGET_DIR=false
if [[ -n "$CUSTOM_TARGET_DIR" ]]; then
    TARGET_DIR="$CUSTOM_TARGET_DIR"
else
    TARGET_DIR="$(mktemp -d "$TMP_ROOT/codelattice-linux-source-target-XXXXXX")"
    AUTO_TARGET_DIR=true
fi

cleanup() {
    if [[ "$AUTO_TARGET_DIR" == "true" && "$KEEP_TEMP" != "true" ]]; then
        rm -rf "$TARGET_DIR"
    elif [[ "$AUTO_TARGET_DIR" == "true" ]]; then
        echo ""
        echo "Keeping target dir: $TARGET_DIR"
    fi
}
trap cleanup EXIT

step() {
    echo ""
    echo "=== $* ==="
}

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "ERROR: required command not found: $1" >&2
        exit 1
    fi
}

have_checksum_tool() {
    command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1
}

step "Platform"
echo "repo:       $REPO_ROOT"
echo "uname:      $(uname -srm 2>/dev/null || echo unknown)"
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    echo "os-release: ${PRETTY_NAME:-unknown}"
else
    echo "os-release: not present"
fi
echo "target dir: $TARGET_DIR"
if [[ -n "$FEATURES" ]]; then
    echo "features:   $FEATURES"
else
    echo "features:   default"
fi

step "Toolchain checks"
for cmd in bash git cargo rustc find sed mktemp tar python3; do
    need_cmd "$cmd"
    echo "OK: $cmd ($(command -v "$cmd"))"
done
if have_checksum_tool; then
    if command -v sha256sum >/dev/null 2>&1; then
        echo "OK: sha256sum ($(command -v sha256sum))"
    else
        echo "OK: shasum ($(command -v shasum))"
    fi
else
    echo "WARN: no sha256sum or shasum found; release checksum verification may fail"
fi
echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"

if [[ "$SKIP_FMT" == "true" ]]; then
    step "Skip formatting check"
    echo "SKIP: requested by --skip-fmt"
else
    step "cargo fmt --check"
    (cd "$REPO_ROOT" && cargo fmt --check)
fi

step "cargo build --release"
build_cmd=(cargo build --release --target-dir "$TARGET_DIR" -p gitnexus-rust-core-cli --bins)
if [[ -n "$FEATURES" ]]; then
    build_cmd+=(--features "$FEATURES")
fi
(cd "$REPO_ROOT" && "${build_cmd[@]}")

BIN="$TARGET_DIR/release/codelattice"
if [[ ! -x "$BIN" ]]; then
    echo "ERROR: built binary not found: $BIN" >&2
    exit 1
fi
echo "binary: $BIN"
"$BIN" --version

if [[ "$SKIP_TESTS" == "true" ]]; then
    step "Skip cargo test --test mcp_server"
    echo "SKIP: requested by --skip-tests"
else
    step "cargo test --test mcp_server"
    # `mcp_smoke_rust_only` shells out to the checkout-local smoke wrapper and
    # is covered by the normal MCP smoke suite. In this isolated source-build
    # target it can fail for environment/path reasons unrelated to whether a
    # source checkout can build and serve MCP, so the portable source-build gate
    # verifies the rest of the MCP test suite plus the explicit CLI/MCP checks
    # below.
    test_cmd=(cargo test --target-dir "$TARGET_DIR" --test mcp_server)
    if [[ -n "$FEATURES" ]]; then
        test_cmd+=(--features "$FEATURES")
    fi
    test_cmd+=(-- --skip mcp_smoke_rust_only)
    (cd "$REPO_ROOT" && "${test_cmd[@]}")
fi

step "Rust fixture analyze"
"$BIN" analyze --root "$REPO_ROOT/fixtures/rust/portable-smoke" --language rust --format json |
    python3 -c '
import json, sys
d = json.load(sys.stdin)
summary = d.get("summary", {})
graph = d.get("graph", {}) if isinstance(d.get("graph", {}), dict) else {}
stats = d.get("stats", graph.get("stats", {}))
counts = {
    "nodes": summary.get("nodeCount", len(graph.get("nodes", d.get("nodes", [])))),
    "edges": summary.get("edgeCount", len(graph.get("edges", d.get("edges", [])))),
    "symbols": stats.get("symbolCount", summary.get("symbolCount", 0)),
    "sourceFiles": stats.get("sourceFileCount", summary.get("sourceFileCount", 0)),
}
if counts["nodes"] <= 0 or counts["symbols"] <= 0 or counts["sourceFiles"] <= 0:
    raise SystemExit("unexpected empty Rust fixture analysis: " + json.dumps(counts))
print(json.dumps(counts, sort_keys=True))
'

step "MCP tools/list"
printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"linux-source-build-smoke","version":"1.0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' |
    env CODELATTICE_MCP_TOOLSET=full "$BIN" mcp 2>/dev/null |
    python3 -c '
import json, sys
tool_count = None
for line in sys.stdin:
    if not line.strip():
        continue
    d = json.loads(line)
    if d.get("id") == 2:
        tool_count = len(d["result"]["tools"])
        break
if tool_count is None:
    raise SystemExit("tools/list response not found")
if tool_count < 50:
    raise SystemExit(f"expected at least 50 tools, got {tool_count}")
print(f"tools: {tool_count}")
'

step "Source build smoke passed"
echo "PASS: source build path is usable on this platform."
