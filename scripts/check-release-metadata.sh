#!/usr/bin/env bash
# Validate CodeLattice release versioning and changelog metadata.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${CODELATTICE_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

FAILURES=0

note_ok() {
    printf 'OK: %s\n' "$1"
}

note_fail() {
    printf 'FAIL: %s\n' "$1" >&2
    FAILURES=$((FAILURES + 1))
}

require_file() {
    local path="$1"
    local label="$2"
    if [[ -f "$REPO_ROOT/$path" ]]; then
        note_ok "$label"
    else
        note_fail "$label missing ($path)"
    fi
}

require_grep() {
    local pattern="$1"
    local path="$2"
    local label="$3"
    if [[ -f "$REPO_ROOT/$path" ]] && grep -Eq "$pattern" "$REPO_ROOT/$path"; then
        note_ok "$label"
    else
        note_fail "$label"
    fi
}

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -1)"
SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$'

echo "=== CodeLattice Release Metadata Check ==="
echo "Repo:    $REPO_ROOT"
echo "Version: ${VERSION:-unknown}"
echo ""

if [[ -n "$VERSION" && "$VERSION" =~ $SEMVER_RE ]]; then
    note_ok "workspace.package.version is SemVer-like"
else
    note_fail "workspace.package.version must be SemVer-like"
fi

require_file "CHANGELOG.md" "CHANGELOG.md"
require_file "docs/release-versioning.md" "release versioning policy"

require_grep '^## \[Unreleased\]' "CHANGELOG.md" "CHANGELOG has an Unreleased section"
if [[ -n "$VERSION" ]]; then
    require_grep "^## \\[$VERSION\\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" "CHANGELOG.md" "CHANGELOG has a dated section for $VERSION"
fi

require_grep 'workspace\.package\.version' "docs/release-versioning.md" "policy names the Cargo workspace version source"
require_grep 'CHANGELOG\.md' "docs/release-versioning.md" "policy names CHANGELOG.md"
require_grep 'MCP serverVersion' "docs/release-versioning.md" "policy separates MCP serverVersion from product version"

require_grep 'CHANGELOG\.md' "scripts/package-release.sh" "package-release includes CHANGELOG.md"
require_grep 'docs/release-versioning\.md' "scripts/package-release.sh" "package-release includes release-versioning.md"
require_grep 'CHANGELOG\.md' "scripts/release-smoke.sh" "release smoke checks CHANGELOG.md"
require_grep 'docs/release-versioning\.md' "scripts/release-smoke.sh" "release smoke checks release-versioning.md"
require_grep 'check-release-metadata\.sh' "docs/release-packaging.md" "release packaging docs include metadata check"

echo ""
if [[ "$FAILURES" -eq 0 ]]; then
    echo "Release metadata check passed."
else
    echo "Release metadata check failed: $FAILURES issue(s)." >&2
    exit 1
fi
