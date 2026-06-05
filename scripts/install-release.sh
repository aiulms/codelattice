#!/usr/bin/env bash
# Install a published CodeLattice release tarball from GitCode.

set -euo pipefail

DEFAULT_VERSION="v0.17.0-beta.1"
DEFAULT_BASE_URL="https://gitcode.com/aiulms/codelattice/releases/download"

VERSION="${CODELATTICE_VERSION:-$DEFAULT_VERSION}"
PLATFORM="${CODELATTICE_PLATFORM:-}"
INSTALL_DIR="${CODELATTICE_TOOL_DIR:-$HOME/.local/share/codelattice-tool}"
BASE_URL="${CODELATTICE_RELEASE_BASE_URL:-$DEFAULT_BASE_URL}"
KEEP_TEMP=false
FORCE=false
DRY_RUN=false

usage() {
    cat <<'HELP'
install-release.sh — Install CodeLattice from a published GitCode Release.

Usage:
  bash scripts/install-release.sh [options]

Options:
  --version <tag>       Release tag or version (default: v0.17.0-beta.1)
  --platform <name>     Platform artifact tag (default: uname-derived)
  --install-dir <path>  Install directory (default: ~/.local/share/codelattice-tool)
  --base-url <url>      Release download base URL
  --force               Allow installing into a non-empty non-CodeLattice directory
  --dry-run             Print resolved URLs and paths without downloading
  --keep-temp           Keep temporary download directory
  --help, -h            Show this help

Environment:
  CODELATTICE_VERSION
  CODELATTICE_PLATFORM
  CODELATTICE_TOOL_DIR
  CODELATTICE_RELEASE_BASE_URL

The installer downloads the tarball and .sha256 file, verifies the checksum,
installs the stable runtime wrapper, and runs codelattice-mcp.sh --self-test.
It never writes Codex, opencode, Claude, or shell configuration files.
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
        --install-dir)
            INSTALL_DIR="${2:-}"
            if [[ -z "$INSTALL_DIR" ]]; then
                echo "ERROR: --install-dir requires a path" >&2
                exit 1
            fi
            shift 2
            ;;
        --base-url)
            BASE_URL="${2:-}"
            if [[ -z "$BASE_URL" ]]; then
                echo "ERROR: --base-url requires a URL" >&2
                exit 1
            fi
            shift 2
            ;;
        --force)
            FORCE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
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

need_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "ERROR: required command not found: $1" >&2
        exit 1
    fi
}

if [[ -z "$PLATFORM" ]]; then
    PLATFORM="$(normalize_platform)"
fi

if [[ "$VERSION" == v* ]]; then
    RELEASE_TAG="$VERSION"
    ARTIFACT_VERSION="${VERSION#v}"
else
    RELEASE_TAG="v$VERSION"
    ARTIFACT_VERSION="$VERSION"
fi

ARTIFACT_NAME="codelattice-${ARTIFACT_VERSION}-${PLATFORM}"
TARBALL="${ARTIFACT_NAME}.tar.gz"
SHA_FILE="${TARBALL}.sha256"
DOWNLOAD_URL="${BASE_URL%/}/${RELEASE_TAG}/${TARBALL}"
SHA_URL="${BASE_URL%/}/${RELEASE_TAG}/${SHA_FILE}"
if [[ "$INSTALL_DIR" != /* ]]; then
    INSTALL_DIR="$(pwd)/$INSTALL_DIR"
fi
INSTALL_DIR="${INSTALL_DIR%/}"

echo "=== CodeLattice Release Installer ==="
echo "Version:     $RELEASE_TAG"
echo "Platform:    $PLATFORM"
echo "Install dir: $INSTALL_DIR"
echo "Tarball:     $DOWNLOAD_URL"
echo "Checksum:    $SHA_URL"
echo ""

if [[ "$DRY_RUN" == "true" ]]; then
    echo "Dry run complete. No files were downloaded or installed."
    exit 0
fi

need_command curl
need_command tar
need_command shasum
need_command python3

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/codelattice-release-install-XXXXXX")"
cleanup() {
    if [[ "$KEEP_TEMP" == "true" ]]; then
        echo "Keeping temp dir: $TMP_ROOT"
    else
        rm -rf "$TMP_ROOT"
    fi
}
trap cleanup EXIT

download_file() {
    local url="$1"
    local out="$2"
    if ! curl -fL --retry 3 --connect-timeout 20 -o "$out" "$url"; then
        echo "ERROR: failed to download: $url" >&2
        echo "This release may not provide platform '$PLATFORM'. Current v0.17.0-beta.1 ships darwin-arm64; other platforms can clone and build from source." >&2
        exit 1
    fi
}

echo "--- Download release artifact ---"
download_file "$DOWNLOAD_URL" "$TMP_ROOT/$TARBALL"
download_file "$SHA_URL" "$TMP_ROOT/$SHA_FILE"

echo ""
echo "--- Verify checksum ---"
(cd "$TMP_ROOT" && shasum -a 256 -c "$SHA_FILE")

echo ""
echo "--- Unpack ---"
tar -xzf "$TMP_ROOT/$TARBALL" -C "$TMP_ROOT"
RELEASE_DIR="$TMP_ROOT/$ARTIFACT_NAME"
if [[ ! -d "$RELEASE_DIR" ]]; then
    echo "ERROR: tarball did not contain expected directory: $ARTIFACT_NAME" >&2
    exit 1
fi

if [[ ! -x "$RELEASE_DIR/codelattice-mcp.sh" ]]; then
    echo "ERROR: release wrapper is missing or not executable" >&2
    exit 1
fi

echo ""
echo "--- Validate unpacked runtime ---"
"$RELEASE_DIR/codelattice-mcp.sh" --self-test

if [[ -d "$INSTALL_DIR" ]] && [[ -n "$(find "$INSTALL_DIR" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]]; then
    if [[ ! -f "$INSTALL_DIR/manifest.json" && "$FORCE" != "true" ]]; then
        echo "ERROR: install directory is non-empty and does not look like a CodeLattice runtime: $INSTALL_DIR" >&2
        echo "Use --force only if this directory is dedicated to CodeLattice." >&2
        exit 1
    fi
    if [[ -f "$INSTALL_DIR/manifest.json" && "$FORCE" != "true" ]]; then
        if ! python3 - "$INSTALL_DIR/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], "r", encoding="utf-8") as f:
    data = json.load(f)
if data.get("name") != "CodeLattice":
    raise SystemExit(1)
PY
        then
            echo "ERROR: install directory manifest is not a CodeLattice runtime: $INSTALL_DIR" >&2
            echo "Use --force only if this directory is dedicated to CodeLattice." >&2
            exit 1
        fi
    fi
fi

echo ""
echo "--- Install stable runtime ---"
mkdir -p "$INSTALL_DIR"
for entry in bin docs fixtures codelattice-mcp.sh manifest.json README.md LICENSE CHANGELOG.md; do
    rm -rf "$INSTALL_DIR/$entry"
done
cp -R "$RELEASE_DIR"/. "$INSTALL_DIR"/
chmod +x "$INSTALL_DIR/codelattice-mcp.sh" "$INSTALL_DIR/bin/codelattice" "$INSTALL_DIR/bin/gitnexus-rust-core-cli"

echo ""
echo "--- Validate installed runtime ---"
"$INSTALL_DIR/codelattice-mcp.sh" --self-test

echo ""
echo "CodeLattice release installed."
echo "  wrapper: $INSTALL_DIR/codelattice-mcp.sh"
echo "  binary:  $INSTALL_DIR/bin/codelattice"
echo ""
echo "Client config command path:"
echo "  $INSTALL_DIR/codelattice-mcp.sh"
