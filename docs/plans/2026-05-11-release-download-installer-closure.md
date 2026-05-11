# CodeLattice Release Download + Installer Closure

Date: 2026-05-11

## Outcome

CodeLattice now has a direct GitCode Release install path and the README no longer claims that the external release process is missing.

## Changes

- Added `scripts/install-release.sh`.
- Added `docs/release-install.md`.
- Updated README Quick Start with a `curl | bash` GitCode Release install path for `v0.1.0`.
- Updated getting-started and release packaging docs with install behavior, options, and platform caveats.
- Updated release packaging and smoke scripts so future tarballs include and verify `docs/release-install.md`.
- Updated release metadata check to require installer docs and script wiring.
- Updated `CHANGELOG.md` under `Unreleased`.

## Installer Behavior

- Defaults to `v0.1.0`.
- Auto-detects platform with `uname`; current published artifact is `darwin-arm64`.
- Downloads release tarball and `.sha256` from GitCode Release.
- Verifies checksum before unpacking.
- Runs `codelattice-mcp.sh --self-test` before and after installation.
- Refuses to overwrite a non-empty non-CodeLattice install directory unless `--force` is provided.
- Does not write AI client configuration files.

## Verification

Passed:

- `bash -n scripts/*.sh`
- `bash scripts/check-release-metadata.sh`
- `bash scripts/install-release.sh --dry-run --version v0.1.0 --platform darwin-arm64 --install-dir /tmp/codelattice-release-install-dry`
- `bash scripts/install-release.sh --version v0.1.0 --platform darwin-arm64 --install-dir /tmp/codelattice-release-install-smoke-*`
- installed wrapper `--self-test`
- `cargo fmt --check`
- `git diff --check`
- `bash scripts/package-release.sh --dist-dir /tmp/codelattice-installer-pack-dist`
- `bash scripts/release-smoke.sh --tarball /tmp/codelattice-installer-pack-dist/codelattice-0.1.0-darwin-arm64.tar.gz`
- `cargo test --test productization_commands`
- `bash scripts/fresh-clone-smoke.sh --skip-tests`
- Tool detect-changes: 14 files, 48 symbols, 0 affected processes, LOW risk
- Tool index refresh: 5,191 nodes, 9,614 edges, 118 clusters, 281 flows

Notes:

- Existing compiler warnings are unchanged and unrelated.
- No real Codex, opencode, Claude, or shell configuration was written.
- No GitCode Release asset was changed in this pack.

## Remaining Non-WebUI Gaps

- Multi-platform release artifacts.
- Release CI automation.
- Package-manager distribution channel.
- External beta install reports from clean machines.
