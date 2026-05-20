#!/usr/bin/env bash
# promote-to-local-tool.sh — Promote a verified CodeLattice build to the local
# stable MCP runtime used by AI IDEs.
#
# This script intentionally separates the development checkout from the runtime
# that Codex/opencode/Claude should point at:
#
#   dev checkout -> explicit promote -> /Users/.../CodeLattice-Tool
#
# It does not edit AI client configuration files. It prints the stable wrapper
# path that clients should use.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$DEFAULT_REPO_ROOT}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
BIN_NAME="codelattice"
COMPAT_BIN_NAME="gitnexus-rust-core-cli"
ALL_LANGUAGE_FEATURES="tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python"
DEFAULT_INSTALL_DIR="${HOME}/Desktop/CodeLattice-Tool"
INSTALL_DIR="${CODELATTICE_TOOL_DIR:-$DEFAULT_INSTALL_DIR}"
DRY_RUN=false
SKIP_BUILD=false
RUN_DOCTOR=true

usage() {
    cat <<'HELP'
promote-to-local-tool.sh — Promote CodeLattice to a stable local MCP runtime.

Usage:
  bash scripts/promote-to-local-tool.sh [options]

Options:
  --install-dir <path>  Install runtime into this directory
                        (default: $HOME/Desktop/CodeLattice-Tool)
  --skip-build          Reuse the existing release binary
  --no-doctor           Skip post-install self-test
  --dry-run             Print actions without changing files
  --help, -h            Show this help

The promoted runtime is self-contained for MCP startup:
  <install-dir>/codelattice-mcp.sh
  <install-dir>/bin/codelattice
  <install-dir>/bin/codelattice-cli
  <install-dir>/manifest.json

AI clients should point at <install-dir>/codelattice-mcp.sh, not this dev repo.
HELP
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --install-dir)
            INSTALL_DIR="${2:-}"
            if [[ -z "$INSTALL_DIR" ]]; then
                echo "ERROR: --install-dir requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --no-doctor)
            RUN_DOCTOR=false
            shift
            ;;
        --dry-run)
            DRY_RUN=true
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

RELEASE_BIN="$REPO_ROOT/target/release/$BIN_NAME"
RELEASE_COMPAT_BIN="$REPO_ROOT/target/release/$COMPAT_BIN_NAME"
INSTALL_BIN_DIR="$INSTALL_DIR/bin"
INSTALL_BIN="$INSTALL_BIN_DIR/codelattice"
INSTALL_LEGACY_ALIAS_BIN="$INSTALL_BIN_DIR/codelattice-cli"
INSTALL_COMPAT_BIN="$INSTALL_BIN_DIR/$COMPAT_BIN_NAME"
INSTALL_WRAPPER="$INSTALL_DIR/codelattice-mcp.sh"
INSTALL_MANIFEST="$INSTALL_DIR/manifest.json"

run() {
    if [[ "$DRY_RUN" == "true" ]]; then
        printf 'DRY-RUN:'
        printf ' %q' "$@"
        printf '\n'
    else
        "$@"
    fi
}

install_executable() {
    local src="$1"
    local dst="$2"
    local tmp="${dst}.new.$$"
    local backup="${dst}.replaced-$(date -u +%Y%m%d%H%M%S)"

    if [[ "$DRY_RUN" == "true" ]]; then
        echo "DRY-RUN: install executable $src -> $dst"
        return
    fi

    cp "$src" "$tmp"
    chmod +x "$tmp"
    if [[ -e "$dst" ]]; then
        # Keep the old inode available for already-running MCP clients. Replacing
        # the executable in place can leave macOS launching the path with SIGKILL
        # while stale clients still map the previous binary.
        mv "$dst" "$backup"
    fi
    mv "$tmp" "$dst"
}

json_escape() {
    python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().rstrip("\n"))[1:-1])'
}

echo "=== CodeLattice Local Tool Promotion ==="
echo "Repo:        $REPO_ROOT"
echo "Install dir: $INSTALL_DIR"
echo ""

if [[ "$SKIP_BUILD" != "true" ]]; then
    echo "--- Building release binary (all language adapters) ---"
    run cargo build --release -p gitnexus-rust-core-cli --features "$ALL_LANGUAGE_FEATURES" --bins --manifest-path "$REPO_ROOT/Cargo.toml"
else
    echo "--- Build skipped ---"
fi

if [[ "$DRY_RUN" != "true" && ! -x "$RELEASE_BIN" ]]; then
    echo "ERROR: release binary not found: $RELEASE_BIN" >&2
    echo "Run without --skip-build first." >&2
    exit 1
fi
if [[ "$DRY_RUN" != "true" && ! -x "$RELEASE_COMPAT_BIN" ]]; then
    echo "ERROR: compatibility binary not found: $RELEASE_COMPAT_BIN" >&2
    echo "Run without --skip-build first." >&2
    exit 1
fi

echo ""
echo "--- Installing stable runtime ---"
run mkdir -p "$INSTALL_BIN_DIR"
install_executable "$RELEASE_BIN" "$INSTALL_BIN"
install_executable "$RELEASE_BIN" "$INSTALL_LEGACY_ALIAS_BIN"
install_executable "$RELEASE_COMPAT_BIN" "$INSTALL_COMPAT_BIN"

if [[ "$DRY_RUN" == "true" ]]; then
    echo "DRY-RUN: write $INSTALL_WRAPPER"
    echo "DRY-RUN: write $INSTALL_MANIFEST"
else
    cat > "$INSTALL_WRAPPER" <<'WRAPPER'
#!/usr/bin/env bash
# Stable CodeLattice MCP runtime wrapper.
# Generated by scripts/promote-to-local-tool.sh.

set -euo pipefail

TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$TOOL_DIR/bin/codelattice"
MANIFEST="$TOOL_DIR/manifest.json"

if [[ ! -x "$BIN" ]]; then
    echo "ERROR: CodeLattice runtime binary not executable: $BIN" >&2
    echo "Fix: re-run scripts/promote-to-local-tool.sh from the CodeLattice dev checkout." >&2
    exit 1
fi

profile_json() {
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"codelattice-tool-profile","version":"1.0"}}}' \
        | env CODELATTICE_MCP_TOOLSET=full "$BIN" mcp 2>/dev/null \
        | python3 -c 'import json, sys
for line in sys.stdin:
    text = line.strip()
    if not text:
        continue
    try:
        doc = json.loads(text)
    except Exception:
        continue
    if doc.get("id") == 1:
        print(json.dumps(doc, separators=(",", ":")))
        break'
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<EOF
codelattice-mcp.sh — Stable CodeLattice MCP runtime

Usage:
  $0              Start MCP stdio server
  $0 --version    Print runtime profile
  $0 --self-test  Run startup diagnostics

Runtime:
  toolDir:  $TOOL_DIR
  binary:   $BIN
  manifest: $MANIFEST
EOF
    exit 0
fi

if [[ "${1:-}" == "--version" ]]; then
    RESP="$(profile_json)"
    echo "codelattice-tool-wrapper"
    echo "  toolDir: $TOOL_DIR"
    echo "  bin:     $BIN"
    if [[ -f "$MANIFEST" ]]; then
        SOURCE_COMMIT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("sourceCommit","unknown"))' "$MANIFEST" 2>/dev/null || echo unknown)"
        INSTALLED_AT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("installedAt","unknown"))' "$MANIFEST" 2>/dev/null || echo unknown)"
        echo "  sourceCommit: $SOURCE_COMMIT"
        echo "  installedAt:  $INSTALLED_AT"
    fi
    echo "$RESP" | python3 -c 'import json,sys
d=json.load(sys.stdin)
s=d["result"]["serverInfo"]
print("  serverVersion: {}".format(s.get("version", "unknown")))
print("  cangjieSupport: {}".format(s.get("cangjieSupport", "unknown")))
print("  arktsSupport: {}".format(s.get("arktsSupport", "unknown")))
print("  typescriptSupport: {}".format(s.get("typescriptSupport", "unknown")))
print("  cSupport: {}".format(s.get("cSupport", "unknown")))
print("  cppSupport: {}".format(s.get("cppSupport", "unknown")))
print("  pythonSupport: {}".format(s.get("pythonSupport", "unknown")))
print("  shellSupport: {}".format(s.get("shellSupport", "unknown")))
print("  toolCount: {}".format(s.get("toolCount", "unknown")))'
    exit 0
fi

if [[ "${1:-}" == "--self-test" ]]; then
    echo "codelattice-tool self-test"
    echo "  toolDir: $TOOL_DIR"
    echo "  bin:     $BIN"
    RESP="$(profile_json)"
    echo "$RESP" | python3 -c 'import json,sys
d=json.load(sys.stdin)
s=d["result"]["serverInfo"]
assert s["name"] == "codelattice"
assert int(s.get("toolCount", 0)) >= 51
assert s.get("cangjieSupport") is True
assert s.get("arktsSupport") is True
assert s.get("typescriptSupport") is True
assert s.get("cSupport") is True
assert s.get("cppSupport") is True
assert s.get("pythonSupport") is True
assert s.get("shellSupport") is True
print("  serverVersion: {}".format(s.get("version")))
print("  toolCount: {}".format(s.get("toolCount")))
print("  cangjieSupport: {}".format(s.get("cangjieSupport")))
print("  arktsSupport: {}".format(s.get("arktsSupport")))
print("  typescriptSupport: {}".format(s.get("typescriptSupport")))
print("  cSupport: {}".format(s.get("cSupport")))
print("  cppSupport: {}".format(s.get("cppSupport")))
print("  pythonSupport: {}".format(s.get("pythonSupport")))
print("  shellSupport: {}".format(s.get("shellSupport")))'

    MULTI_RESP="$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"codelattice-tool-self-test","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_cache_status","arguments":{}}}\n' | env CODELATTICE_MCP_TOOLSET=full "$BIN" mcp 2>/dev/null)"
    TOOL_COUNT="$(echo "$MULTI_RESP" | python3 -c 'import json,sys
for line in sys.stdin:
    if not line.strip():
        continue
    d=json.loads(line)
    if d.get("id") == 2:
        print(len(d["result"]["tools"]))
        break')"
    if [[ "$TOOL_COUNT" -lt 51 ]]; then
        echo "FAIL: tools/list returned $TOOL_COUNT tools" >&2
        exit 1
    fi
    echo "  tools/list: OK ($TOOL_COUNT tools)"
    echo "Self-test passed."
    exit 0
fi

exec "$BIN" mcp
WRAPPER
    chmod +x "$INSTALL_WRAPPER"

    SOURCE_COMMIT="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    SOURCE_REMOTE="$(git -C "$REPO_ROOT" remote get-url gitcode 2>/dev/null || git -C "$REPO_ROOT" remote get-url origin 2>/dev/null || echo unknown)"
    INSTALLED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    BINARY_SHA256="$(shasum -a 256 "$INSTALL_BIN" | awk '{print $1}')"
    INIT_RESP="$(printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"promote-manifest","version":"1.0"}}}' | env CODELATTICE_MCP_TOOLSET=full "$INSTALL_BIN" mcp 2>/dev/null | python3 -c 'import json, sys
for line in sys.stdin:
    text=line.strip()
    if not text:
        continue
    try:
        doc=json.loads(text)
    except Exception:
        continue
    if doc.get("id") == 1:
        print(json.dumps(doc, separators=(",", ":")))
        break')"
    SERVER_VERSION="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("version","unknown"))' 2>/dev/null || echo unknown)"
    CANGJIE_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cangjieSupport", False)).lower())' 2>/dev/null || echo false)"
    ARKTS_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("arktsSupport", False)).lower())' 2>/dev/null || echo false)"
    TYPESCRIPT_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("typescriptSupport", False)).lower())' 2>/dev/null || echo false)"
    C_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cSupport", False)).lower())' 2>/dev/null || echo false)"
    CPP_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cppSupport", False)).lower())' 2>/dev/null || echo false)"
    PYTHON_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("pythonSupport", False)).lower())' 2>/dev/null || echo false)"
    SHELL_SUPPORT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("shellSupport", False)).lower())' 2>/dev/null || echo false)"
    TOOL_COUNT="$(echo "$INIT_RESP" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("toolCount",0))' 2>/dev/null || echo 0)"
    cat > "$INSTALL_MANIFEST" <<JSON
{
  "name": "CodeLattice-Tool",
  "layoutVersion": 1,
  "sourceRepo": "$(printf '%s' "$REPO_ROOT" | json_escape)",
  "sourceRemote": "$(printf '%s' "$SOURCE_REMOTE" | json_escape)",
  "sourceCommit": "$(printf '%s' "$SOURCE_COMMIT" | json_escape)",
  "installedAt": "$INSTALLED_AT",
  "binary": "bin/codelattice",
  "legacyAliasBinary": "bin/codelattice-cli",
  "compatBinary": "bin/$COMPAT_BIN_NAME",
  "binarySha256": "$BINARY_SHA256",
  "wrapper": "codelattice-mcp.sh",
  "paths": {
    "binary": "bin/codelattice",
    "legacyAliasBinary": "bin/codelattice-cli",
    "compatBinary": "bin/$COMPAT_BIN_NAME",
    "wrapper": "codelattice-mcp.sh",
    "manifest": "manifest.json"
  },
  "profile": {
    "serverVersion": "$SERVER_VERSION",
    "cangjieSupport": $CANGJIE_SUPPORT,
    "arktsSupport": $ARKTS_SUPPORT,
    "typescriptSupport": $TYPESCRIPT_SUPPORT,
    "cSupport": $C_SUPPORT,
    "cppSupport": $CPP_SUPPORT,
    "pythonSupport": $PYTHON_SUPPORT,
    "shellSupport": $SHELL_SUPPORT,
    "toolCount": $TOOL_COUNT
  }
}
JSON
fi

echo ""
echo "--- Stable runtime ready ---"
echo "Wrapper: $INSTALL_WRAPPER"
echo "Binary:  $INSTALL_BIN"
echo ""
echo "Codex config:"
cat <<TOML
[mcp_servers.codelattice]
type = "stdio"
command = "bash"
args = ["$INSTALL_WRAPPER"]
TOML
echo ""
echo "opencode config command:"
echo "  \"$INSTALL_WRAPPER\""

if [[ "$RUN_DOCTOR" == "true" && "$DRY_RUN" != "true" ]]; then
    echo ""
    echo "--- Runtime doctor ---"
    "$INSTALL_WRAPPER" --version
    "$INSTALL_WRAPPER" --self-test
fi
