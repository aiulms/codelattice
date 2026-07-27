#!/usr/bin/env bash
# fresh-clone-smoke.sh — Verify the external fresh-clone MCP install path.
#
# This script does not clone from the network. It copies the current checkout to
# /tmp while excluding local build/index/client artifacts, then verifies that the
# copied tree can build, promote a stable MCP runtime, and serve fixture analysis.

set -euo pipefail

usage() {
    cat <<'HELP'
fresh-clone-smoke.sh — Simulate a fresh external clone and MCP install.

Usage:
  bash scripts/fresh-clone-smoke.sh [options]

Options:
  --keep-temp          Keep temporary fresh clone and install dir
  --skip-tests         Skip cargo test --test mcp_server in the fresh clone
  --install-dir <path> Promote the temporary runtime into this directory
  --help, -h           Show this help

The script never writes Codex, opencode, Claude, or other client configs.
HELP
}

KEEP_TEMP=false
SKIP_TESTS=false
CUSTOM_INSTALL_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep-temp)
            KEEP_TEMP=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --install-dir)
            CUSTOM_INSTALL_DIR="${2:-}"
            if [[ -z "$CUSTOM_INSTALL_DIR" ]]; then
                echo "ERROR: --install-dir requires a path" >&2
                exit 1
            fi
            shift 2
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
SOURCE_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
SOURCE_ROOT="$(cd "$SOURCE_ROOT" && pwd)"

if [[ ! -f "$SOURCE_ROOT/Cargo.toml" ]]; then
    echo "ERROR: CODELATTICE_ROOT does not look like a CodeLattice checkout: $SOURCE_ROOT" >&2
    exit 1
fi

TMP_ROOT="${TMPDIR:-/tmp}"
FRESH_PARENT="$(mktemp -d "$TMP_ROOT/codelattice-fresh-smoke-XXXXXX")"
FRESH_DIR="$FRESH_PARENT/codelattice"

INSTALL_DIR_AUTO=false
if [[ -n "$CUSTOM_INSTALL_DIR" ]]; then
    INSTALL_DIR="$CUSTOM_INSTALL_DIR"
else
    INSTALL_DIR="$(mktemp -d "$TMP_ROOT/codelattice-tool-smoke-XXXXXX")"
    INSTALL_DIR_AUTO=true
fi

cleanup() {
    if [[ "$KEEP_TEMP" == "true" ]]; then
        echo ""
        echo "Keeping temporary paths:"
        echo "  fresh clone: $FRESH_DIR"
        echo "  install dir: $INSTALL_DIR"
        return
    fi
    rm -rf "$FRESH_PARENT"
    if [[ "$INSTALL_DIR_AUTO" == "true" ]]; then
        rm -rf "$INSTALL_DIR"
    fi
}
trap cleanup EXIT

step() {
    echo ""
    echo "=== $* ==="
}

run_in_fresh() {
    (cd "$FRESH_DIR" && "$@")
}

json_rpc() {
    local wrapper="$1"
    local payload="$2"
    printf '%s\n' "$payload" | env CODELATTICE_MCP_TOOLSET=full "$wrapper" 2>/dev/null
}

check_tools_list() {
    local wrapper="$1"
    local count
    count=$(json_rpc "$wrapper" '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fresh-clone-smoke","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | python3 -c '
import json, sys
for line in sys.stdin:
    if not line.strip():
        continue
    d = json.loads(line)
    if d.get("id") == 2:
        print(len(d["result"]["tools"]))
        break
')
    if [[ "${count:-0}" -lt 50 ]]; then
        echo "FAIL: tools/list returned ${count:-0} tools, expected >= 50" >&2
        exit 1
    fi
    echo "PASS: tools/list returned $count tools"
}

check_project_overview() {
    local wrapper="$1"
    local fixture_root="$2"
    local language="$3"
    local label="$4"
    local payload
    payload=$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fresh-clone-smoke","version":"1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"codelattice_project_overview","arguments":{"root":"%s","language":"%s"}}}' "$fixture_root" "$language")
    json_rpc "$wrapper" "$payload" | python3 -c '
import json, sys
for line in sys.stdin:
    if not line.strip():
        continue
    d = json.loads(line)
    if d.get("id") != 2:
        continue
    if d.get("result", {}).get("isError"):
        raise SystemExit("tool returned error: " + json.dumps(d, ensure_ascii=False))
    text = d["result"]["content"][0]["text"]
    data = json.loads(text)
    counts = {
        "nodeCount": data.get("nodeCount", 0),
        "edgeCount": data.get("edgeCount", 0),
        "symbolCount": data.get("symbolCount", 0),
        "sourceFileCount": data.get("sourceFileCount", 0),
    }
    missing = [k for k, v in counts.items() if int(v) <= 0]
    if missing:
        raise SystemExit("nonzero count check failed: " + json.dumps(counts, ensure_ascii=False))
    print(json.dumps(counts, ensure_ascii=False))
    break
' | sed "s/^/PASS: $label project_overview /"
}

detect_language_support() {
    local wrapper="$1"
    local support_key="$2"
    json_rpc "$wrapper" '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fresh-clone-smoke","version":"1.0"}}}' | python3 -c '
import json, sys
support_key = sys.argv[1]
for line in sys.stdin:
    if not line.strip():
        continue
    d = json.loads(line)
    if d.get("id") == 1:
        print("true" if d["result"]["serverInfo"].get(support_key) is True else "false")
        break
' "$support_key"
}

step "Copy current checkout to a fresh temp tree"
mkdir -p "$FRESH_DIR"
rsync -a --delete \
    --exclude 'target/' \
    --exclude '.git/' \
    --exclude '.gitnexus/' \
    --exclude '.claude/' \
    --exclude '.agents/' \
    --exclude 'CodeLattice-Tool/' \
    --exclude '*.bridge.json' \
    --exclude 'bridge-*.json' \
    --exclude '*.tmp.json' \
    "$SOURCE_ROOT/" "$FRESH_DIR/"
echo "Fresh clone: $FRESH_DIR"
echo "Install dir: $INSTALL_DIR"

step "Run fresh clone formatting check"
run_in_fresh cargo fmt --check

if [[ "$SKIP_TESTS" == "true" ]]; then
    step "Skip cargo test --test mcp_server"
    echo "SKIP: requested by --skip-tests"
else
    step "Run fresh clone MCP server smoke subset"
    echo "INFO: fresh copy excludes .git by design; skipping mcp_smoke_rust_only because alpha Tool import requires a git repo."
    run_in_fresh cargo test --test mcp_server mcp_initialize_returns_capabilities
    run_in_fresh cargo test --test mcp_server mcp_tools_list_returns_thirty_eight_tools
    run_in_fresh cargo test --test mcp_server mcp_analyze_rust_portable_smoke
    run_in_fresh cargo test --test mcp_server mcp_project_overview_rust
fi

step "Build release MCP binary in fresh clone"
run_in_fresh bash scripts/install-mcp.sh --install-dir "$INSTALL_DIR" --build

step "Promote fresh clone to temporary stable runtime"
run_in_fresh bash scripts/promote-to-local-tool.sh --install-dir "$INSTALL_DIR"

PROMOTED_WRAPPER="$INSTALL_DIR/codelattice-mcp.sh"
if [[ ! -x "$PROMOTED_WRAPPER" ]]; then
    echo "FAIL: promoted wrapper missing: $PROMOTED_WRAPPER" >&2
    exit 1
fi

step "Run promoted wrapper self-test"
"$PROMOTED_WRAPPER" --self-test

step "Check MCP tools/list"
check_tools_list "$PROMOTED_WRAPPER"

step "Run Rust portable fixture project_overview"
check_project_overview "$PROMOTED_WRAPPER" "$FRESH_DIR/fixtures/rust/portable-smoke" "rust" "Rust"

step "Run Cangjie fixture smoke when supported"
CANGJIE_SUPPORT="$(detect_language_support "$PROMOTED_WRAPPER" "cangjieSupport")"
if [[ "$CANGJIE_SUPPORT" == "true" && -d "$FRESH_DIR/fixtures/cangjie/portable-smoke" ]]; then
    check_project_overview "$PROMOTED_WRAPPER" "$FRESH_DIR/fixtures/cangjie/portable-smoke" "cangjie" "Cangjie"
else
    echo "SKIP: Cangjie fixture smoke (support=$CANGJIE_SUPPORT)"
fi

step "Run C fixture smoke when supported"
C_SUPPORT="$(detect_language_support "$PROMOTED_WRAPPER" "cSupport")"
if [[ "$C_SUPPORT" == "true" && -d "$FRESH_DIR/fixtures/c/portable-smoke" ]]; then
    check_project_overview "$PROMOTED_WRAPPER" "$FRESH_DIR/fixtures/c/portable-smoke" "c" "C"
else
    echo "SKIP: C fixture smoke (support=$C_SUPPORT)"
fi

step "Run C++ fixture smoke when supported"
CPP_SUPPORT="$(detect_language_support "$PROMOTED_WRAPPER" "cppSupport")"
if [[ "$CPP_SUPPORT" == "true" && -d "$FRESH_DIR/fixtures/cpp/portable-smoke" ]]; then
    check_project_overview "$PROMOTED_WRAPPER" "$FRESH_DIR/fixtures/cpp/portable-smoke" "cpp" "C++"
else
    echo "SKIP: C++ fixture smoke (support=$CPP_SUPPORT)"
fi

step "Run Python fixture smoke when supported"
PYTHON_SUPPORT="$(detect_language_support "$PROMOTED_WRAPPER" "pythonSupport")"
if [[ "$PYTHON_SUPPORT" == "true" && -d "$FRESH_DIR/fixtures/python/portable-smoke" ]]; then
    check_project_overview "$PROMOTED_WRAPPER" "$FRESH_DIR/fixtures/python/portable-smoke" "python" "Python"
else
    echo "SKIP: Python fixture smoke (support=$PYTHON_SUPPORT)"
fi

echo ""
echo "Fresh clone smoke passed."
