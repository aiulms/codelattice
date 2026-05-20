#!/usr/bin/env bash
# Validate a packaged CodeLattice release tarball from a clean temp directory.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
DIST_DIR="$REPO_ROOT/dist"
TARBALL=""
KEEP_TEMP=false

usage() {
    cat <<'HELP'
release-smoke.sh — Validate a CodeLattice release tarball.

Usage:
  bash scripts/release-smoke.sh [options]

Options:
  --tarball <path>  Release tarball to smoke-test
  --keep-temp       Keep unpacked temp directory for debugging
  --help, -h        Show this help

If --tarball is omitted, the newest dist/codelattice-*.tar.gz is used.
The script never writes AI client configuration files.
HELP
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tarball)
            TARBALL="${2:-}"
            if [[ -z "$TARBALL" ]]; then
                echo "ERROR: --tarball requires a path" >&2
                exit 1
            fi
            shift 2
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

if [[ -z "$TARBALL" ]]; then
    TARBALL="$(find "$DIST_DIR" -maxdepth 1 -type f -name 'codelattice-*.tar.gz' 2>/dev/null | sort | tail -1 || true)"
fi

if [[ -z "$TARBALL" || ! -f "$TARBALL" ]]; then
    echo "ERROR: no release tarball found." >&2
    echo "Run: bash scripts/package-release.sh" >&2
    exit 1
fi

TARBALL="$(cd "$(dirname "$TARBALL")" && pwd)/$(basename "$TARBALL")"
SHA_FILE="$TARBALL.sha256"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/codelattice-release-smoke-XXXXXX")"

cleanup() {
    if [[ "$KEEP_TEMP" == "true" ]]; then
        echo "Keeping temp dir: $TMP_ROOT"
    else
        rm -rf "$TMP_ROOT"
    fi
}
trap cleanup EXIT

echo "=== CodeLattice Release Smoke ==="
echo "Tarball: $TARBALL"
echo "Temp:    $TMP_ROOT"
echo ""

if [[ -f "$SHA_FILE" ]]; then
    echo "--- Checksum ---"
    (cd "$(dirname "$TARBALL")" && shasum -a 256 -c "$(basename "$SHA_FILE")")
    echo ""
fi

echo "--- Unpack ---"
tar -xzf "$TARBALL" -C "$TMP_ROOT"
RELEASE_DIR="$(find "$TMP_ROOT" -mindepth 1 -maxdepth 1 -type d | sort | head -1)"
if [[ -z "$RELEASE_DIR" || ! -d "$RELEASE_DIR" ]]; then
    echo "ERROR: tarball did not contain a top-level release directory" >&2
    exit 1
fi
echo "Release dir: $RELEASE_DIR"

BIN="$RELEASE_DIR/bin/codelattice"
COMPAT_BIN="$RELEASE_DIR/bin/gitnexus-rust-core-cli"
WRAPPER="$RELEASE_DIR/codelattice-mcp.sh"
MANIFEST="$RELEASE_DIR/manifest.json"
CHANGELOG="$RELEASE_DIR/CHANGELOG.md"
RELEASE_POLICY="$RELEASE_DIR/docs/release-versioning.md"
RELEASE_INSTALL="$RELEASE_DIR/docs/release-install.md"
RUST_FIXTURE="$RELEASE_DIR/fixtures/rust/portable-smoke"
CANGJIE_FIXTURE="$RELEASE_DIR/fixtures/cangjie/portable-smoke"
ARKTS_FIXTURE="$RELEASE_DIR/fixtures/arkts/portable-smoke"
TYPESCRIPT_FIXTURE="$RELEASE_DIR/fixtures/typescript/portable-smoke"
C_FIXTURE="$RELEASE_DIR/fixtures/c/portable-smoke"
CPP_FIXTURE="$RELEASE_DIR/fixtures/cpp/portable-smoke"
PYTHON_FIXTURE="$RELEASE_DIR/fixtures/python/portable-smoke"

for path in "$BIN" "$COMPAT_BIN" "$WRAPPER" "$MANIFEST" "$CHANGELOG" "$RELEASE_POLICY" "$RELEASE_INSTALL"; do
    if [[ ! -e "$path" ]]; then
        echo "ERROR: expected release file missing: $path" >&2
        exit 1
    fi
done
if [[ ! -x "$BIN" || ! -x "$COMPAT_BIN" || ! -x "$WRAPPER" ]]; then
    echo "ERROR: release binary/wrapper is not executable" >&2
    exit 1
fi

echo ""
echo "--- Version ---"
"$BIN" --version
"$COMPAT_BIN" --version

echo ""
echo "--- Wrapper self-test ---"
"$WRAPPER" --self-test

echo ""
echo "--- MCP tools/list ---"
TOOLS_COUNT="$(printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"release-smoke","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' \
    | env CODELATTICE_MCP_TOOLSET=full "$WRAPPER" 2>/dev/null \
    | python3 -c 'import json,sys
for line in sys.stdin:
    if not line.strip():
        continue
    msg=json.loads(line)
    if msg.get("id") == 2:
        print(len(msg["result"]["tools"]))
        break')"
if [[ -z "$TOOLS_COUNT" || "$TOOLS_COUNT" -lt 50 ]]; then
    echo "ERROR: tools/list returned ${TOOLS_COUNT:-0} tools" >&2
    exit 1
fi
echo "tools/list: OK ($TOOLS_COUNT tools)"

echo ""
echo "--- Language support profile ---"
PROFILE_JSON="$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"release-smoke-profile","version":"1.0"}}}' \
    | "$BIN" mcp 2>/dev/null \
    | head -1)"
echo "$PROFILE_JSON" | python3 -c 'import json,sys
d=json.load(sys.stdin)
s=d["result"]["serverInfo"]
assert s.get("cangjieSupport") is True, "cangjieSupport must be true in release artifact"
assert s.get("arktsSupport") is True, "arktsSupport must be true in release artifact"
assert s.get("typescriptSupport") is True, "typescriptSupport must be true in release artifact"
assert s.get("cSupport") is True, "cSupport must be true in release artifact"
assert s.get("cppSupport") is True, "cppSupport must be true in release artifact"
assert s.get("pythonSupport") is True, "pythonSupport must be true in release artifact"
print("language support: OK cangjie={} arkts={} typescript={} c={} cpp={} python={}".format(
    s.get("cangjieSupport"),
    s.get("arktsSupport"),
    s.get("typescriptSupport"),
    s.get("cSupport"),
    s.get("cppSupport"),
    s.get("pythonSupport"),
))'

echo ""
echo "--- Rust fixture analyze ---"
if [[ ! -d "$RUST_FIXTURE" ]]; then
    echo "ERROR: missing packaged Rust fixture: $RUST_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$RUST_FIXTURE" --language rust --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "rust"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("rust: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "--- Cangjie fixture analyze ---"
CANGJIE_SUPPORT="$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"release-smoke-profile","version":"1.0"}}}' \
    | "$BIN" mcp 2>/dev/null \
    | head -1 \
    | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cangjieSupport", False)).lower())')"
if [[ "$CANGJIE_SUPPORT" == "true" && -d "$CANGJIE_FIXTURE" ]]; then
    "$BIN" analyze --root "$CANGJIE_FIXTURE" --language cangjie --format json \
        | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "cangjie"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("cangjie: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'
else
    echo "cangjie: SKIP (support=$CANGJIE_SUPPORT fixture=$([[ -d "$CANGJIE_FIXTURE" ]] && echo present || echo missing))"
fi

echo ""
echo "--- ArkTS fixture analyze ---"
if [[ ! -d "$ARKTS_FIXTURE" ]]; then
    echo "ERROR: missing packaged ArkTS fixture: $ARKTS_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$ARKTS_FIXTURE" --language arkts --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "arkts"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("arkts: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "--- TypeScript fixture analyze ---"
if [[ ! -d "$TYPESCRIPT_FIXTURE" ]]; then
    echo "ERROR: missing packaged TypeScript fixture: $TYPESCRIPT_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$TYPESCRIPT_FIXTURE" --language typescript --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "typescript"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("typescript: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "--- C fixture analyze ---"
if [[ ! -d "$C_FIXTURE" ]]; then
    echo "ERROR: missing packaged C fixture: $C_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$C_FIXTURE" --language c --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "c"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("c: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "--- C++ fixture analyze ---"
if [[ ! -d "$CPP_FIXTURE" ]]; then
    echo "ERROR: missing packaged C++ fixture: $CPP_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$CPP_FIXTURE" --language cpp --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "cpp"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("cpp: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "--- Python fixture analyze ---"
if [[ ! -d "$PYTHON_FIXTURE" ]]; then
    echo "ERROR: missing packaged Python fixture: $PYTHON_FIXTURE" >&2
    exit 1
fi
"$BIN" analyze --root "$PYTHON_FIXTURE" --language python --format json \
    | python3 -c 'import json,sys
d=json.load(sys.stdin)
assert d["language"] == "python"
assert d["summary"]["symbolCount"] > 0
assert d["summary"]["sourceFileCount"] > 0
assert d["summary"]["edgeCount"] > 0
print("python: OK symbols={} files={} edges={}".format(
    d["summary"]["symbolCount"],
    d["summary"]["sourceFileCount"],
    d["summary"]["edgeCount"],
))'

echo ""
echo "Release smoke passed."
