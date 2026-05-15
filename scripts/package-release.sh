#!/usr/bin/env bash
# Build a portable CodeLattice release tarball.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
DIST_DIR="$REPO_ROOT/dist"
VERSION=""
PLATFORM=""
SKIP_BUILD=false
KEEP_TEMP=false

usage() {
    cat <<'HELP'
package-release.sh — Build a portable CodeLattice release tarball.

Usage:
  bash scripts/package-release.sh [options]

Options:
  --version <version>   Override release version (default: workspace.package.version)
  --platform <name>     Override platform tag (default: uname-derived)
  --dist-dir <path>     Output directory (default: ./dist)
  --skip-build          Reuse existing target/release binaries
  --keep-temp           Keep staging directory for debugging
  --help, -h            Show this help

Output:
  dist/codelattice-<version>-<platform>.tar.gz
  dist/codelattice-<version>-<platform>.tar.gz.sha256

The package includes:
  bin/codelattice
  bin/gitnexus-rust-core-cli
  codelattice-mcp.sh
  manifest.json
  README.md
  CHANGELOG.md
  docs/getting-started.md, docs/release-install.md, docs/release-versioning.md, and docs/release-packaging.md when present
  portable Rust/Cangjie fixtures for release smoke
HELP
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="${2:-}"
            if [[ -z "$VERSION" ]]; then
                echo "ERROR: --version requires a value" >&2
                exit 1
            fi
            shift 2
            ;;
        --platform)
            PLATFORM="${2:-}"
            if [[ -z "$PLATFORM" ]]; then
                echo "ERROR: --platform requires a value" >&2
                exit 1
            fi
            shift 2
            ;;
        --dist-dir)
            DIST_DIR="${2:-}"
            if [[ -z "$DIST_DIR" ]]; then
                echo "ERROR: --dist-dir requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD=true
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

if [[ -z "$VERSION" ]]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -1)"
fi
if [[ -z "$VERSION" ]]; then
    echo "ERROR: could not determine workspace version from Cargo.toml" >&2
    exit 1
fi

normalize_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m | tr '[:upper:]' '[:lower:]')"
    case "$os" in
        darwin) os="darwin" ;;
        linux) os="linux" ;;
        *) os="${os// /-}" ;;
    esac
    case "$arch" in
        arm64|aarch64) arch="arm64" ;;
        x86_64|amd64) arch="x86_64" ;;
        *) arch="${arch// /-}" ;;
    esac
    printf '%s-%s\n' "$os" "$arch"
}

if [[ -z "$PLATFORM" ]]; then
    PLATFORM="$(normalize_platform)"
fi

ARTIFACT_NAME="codelattice-${VERSION}-${PLATFORM}"
DIST_DIR="$(mkdir -p "$DIST_DIR" && cd "$DIST_DIR" && pwd)"
TARBALL="$DIST_DIR/${ARTIFACT_NAME}.tar.gz"
SHA_FILE="$TARBALL.sha256"
STAGE_PARENT="$(mktemp -d "${TMPDIR:-/tmp}/codelattice-package-XXXXXX")"
STAGE_DIR="$STAGE_PARENT/$ARTIFACT_NAME"

cleanup() {
    if [[ "$KEEP_TEMP" == "true" ]]; then
        echo "Keeping stage dir: $STAGE_PARENT"
    else
        rm -rf "$STAGE_PARENT"
    fi
}
trap cleanup EXIT

echo "=== CodeLattice Release Packaging ==="
echo "Repo:     $REPO_ROOT"
echo "Version:  $VERSION"
echo "Platform: $PLATFORM"
echo "Dist:     $DIST_DIR"
echo ""

if [[ "$SKIP_BUILD" != "true" ]]; then
    echo "--- Build release binaries (Rust + Cangjie) ---"
    cargo build \
        --release \
        --manifest-path "$REPO_ROOT/Cargo.toml" \
        -p gitnexus-rust-core-cli \
        --features tree-sitter-cangjie \
        --bins
else
    echo "--- Build skipped ---"
fi

BIN_CODELATTICE="$REPO_ROOT/target/release/codelattice"
BIN_COMPAT="$REPO_ROOT/target/release/gitnexus-rust-core-cli"
if [[ ! -x "$BIN_CODELATTICE" ]]; then
    echo "ERROR: missing release binary: $BIN_CODELATTICE" >&2
    exit 1
fi
if [[ ! -x "$BIN_COMPAT" ]]; then
    echo "ERROR: missing compatibility binary: $BIN_COMPAT" >&2
    exit 1
fi

echo ""
echo "--- Stage artifact ---"
mkdir -p "$STAGE_DIR/bin" "$STAGE_DIR/docs" "$STAGE_DIR/fixtures/rust" "$STAGE_DIR/fixtures/cangjie"
cp "$BIN_CODELATTICE" "$STAGE_DIR/bin/codelattice"
cp "$BIN_COMPAT" "$STAGE_DIR/bin/gitnexus-rust-core-cli"
chmod +x "$STAGE_DIR/bin/codelattice" "$STAGE_DIR/bin/gitnexus-rust-core-cli"

cp "$REPO_ROOT/README.md" "$STAGE_DIR/README.md"
cp "$REPO_ROOT/CHANGELOG.md" "$STAGE_DIR/CHANGELOG.md"
if [[ -f "$REPO_ROOT/LICENSE" ]]; then
    cp "$REPO_ROOT/LICENSE" "$STAGE_DIR/LICENSE"
fi
for doc in docs/getting-started.md docs/release-install.md docs/release-versioning.md docs/release-packaging.md docs/architecture/mcp-local-client-setup.md docs/architecture/mcp-v0-contract.md; do
    if [[ -f "$REPO_ROOT/$doc" ]]; then
        mkdir -p "$STAGE_DIR/$(dirname "$doc")"
        cp "$REPO_ROOT/$doc" "$STAGE_DIR/$doc"
    fi
done

cp -R "$REPO_ROOT/fixtures/rust/portable-smoke" "$STAGE_DIR/fixtures/rust/portable-smoke"
cp -R "$REPO_ROOT/fixtures/cangjie/portable-smoke" "$STAGE_DIR/fixtures/cangjie/portable-smoke"

cat > "$STAGE_DIR/codelattice-mcp.sh" <<'WRAPPER'
#!/usr/bin/env bash
# Stable CodeLattice MCP runtime wrapper shipped in release tarballs.

set -euo pipefail

TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$TOOL_DIR/bin/codelattice"
MANIFEST="$TOOL_DIR/manifest.json"

if [[ ! -x "$BIN" ]]; then
    echo "ERROR: CodeLattice release binary not executable: $BIN" >&2
    exit 1
fi

profile_json() {
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"codelattice-release-profile","version":"1.0"}}}' \
        | "$BIN" mcp 2>/dev/null \
        | head -1
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<EOF
codelattice-mcp.sh — CodeLattice release MCP runtime

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
    echo "codelattice-release-wrapper"
    echo "  toolDir: $TOOL_DIR"
    echo "  bin:     $BIN"
    if [[ -f "$MANIFEST" ]]; then
        SOURCE_COMMIT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("sourceCommit","unknown"))' "$MANIFEST" 2>/dev/null || echo unknown)"
        echo "  sourceCommit: $SOURCE_COMMIT"
    fi
    echo "$RESP" | python3 -c 'import json,sys
d=json.load(sys.stdin)
s=d["result"]["serverInfo"]
print("  serverVersion: {}".format(s.get("version", "unknown")))
print("  cangjieSupport: {}".format(s.get("cangjieSupport", "unknown")))
print("  toolCount: {}".format(s.get("toolCount", "unknown")))'
    exit 0
fi

if [[ "${1:-}" == "--self-test" ]]; then
    echo "codelattice release self-test"
    echo "  toolDir: $TOOL_DIR"
    echo "  bin:     $BIN"
    RESP="$(profile_json)"
    echo "$RESP" | python3 -c 'import json,sys
d=json.load(sys.stdin)
s=d["result"]["serverInfo"]
assert s["name"] == "codelattice"
assert int(s.get("toolCount", 0)) >= 21
assert s.get("cangjieSupport") is True
print("  serverVersion: {}".format(s.get("version")))
print("  toolCount: {}".format(s.get("toolCount")))
print("  cangjieSupport: {}".format(s.get("cangjieSupport")))'
    echo "Self-test passed."
    exit 0
fi

exec "$BIN" mcp
WRAPPER
chmod +x "$STAGE_DIR/codelattice-mcp.sh"

SOURCE_COMMIT="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
SOURCE_REMOTE="$(git -C "$REPO_ROOT" remote get-url gitcode 2>/dev/null || git -C "$REPO_ROOT" remote get-url origin 2>/dev/null || echo unknown)"
PACKAGED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
BINARY_SHA256="$(shasum -a 256 "$STAGE_DIR/bin/codelattice" | awk '{print $1}')"
COMPAT_SHA256="$(shasum -a 256 "$STAGE_DIR/bin/gitnexus-rust-core-cli" | awk '{print $1}')"
PROFILE_JSON="$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"package-release","version":"1.0"}}}' | "$STAGE_DIR/bin/codelattice" mcp 2>/dev/null | head -1)"
SERVER_VERSION="$(echo "$PROFILE_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("version","unknown"))')"
CANGJIE_SUPPORT="$(echo "$PROFILE_JSON" | python3 -c 'import json,sys; print(str(json.load(sys.stdin)["result"]["serverInfo"].get("cangjieSupport", False)).lower())')"
TOOL_COUNT="$(echo "$PROFILE_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["serverInfo"].get("toolCount",0))')"

cat > "$STAGE_DIR/manifest.json" <<JSON
{
  "name": "CodeLattice",
  "layoutVersion": 1,
  "version": "$VERSION",
  "platform": "$PLATFORM",
  "artifact": "$ARTIFACT_NAME",
  "sourceRemote": "$SOURCE_REMOTE",
  "sourceCommit": "$SOURCE_COMMIT",
  "packagedAt": "$PACKAGED_AT",
  "paths": {
    "binary": "bin/codelattice",
    "compatBinary": "bin/gitnexus-rust-core-cli",
    "wrapper": "codelattice-mcp.sh",
    "manifest": "manifest.json",
    "changelog": "CHANGELOG.md",
    "releasePolicy": "docs/release-versioning.md",
    "rustFixture": "fixtures/rust/portable-smoke",
    "cangjieFixture": "fixtures/cangjie/portable-smoke"
  },
  "checksums": {
    "binarySha256": "$BINARY_SHA256",
    "compatBinarySha256": "$COMPAT_SHA256"
  },
  "profile": {
    "serverVersion": "$SERVER_VERSION",
    "cangjieSupport": $CANGJIE_SUPPORT,
    "toolCount": $TOOL_COUNT
  }
}
JSON

echo "Staged: $STAGE_DIR"

echo ""
echo "--- Create tarball ---"
rm -f "$TARBALL" "$SHA_FILE"
tar -czf "$TARBALL" -C "$STAGE_PARENT" "$ARTIFACT_NAME"
(cd "$DIST_DIR" && shasum -a 256 "$(basename "$TARBALL")" > "$(basename "$SHA_FILE")")

echo "Tarball:  $TARBALL"
echo "Checksum: $SHA_FILE"
echo ""
echo "Next:"
echo "  bash scripts/release-smoke.sh --tarball \"$TARBALL\""
