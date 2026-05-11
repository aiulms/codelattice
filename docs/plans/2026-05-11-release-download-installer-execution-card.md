# CodeLattice Release Download + Installer Execution Card

Date: 2026-05-11

## Write Set

- `scripts/install-release.sh`
- `scripts/check-release-metadata.sh`
- `README.md`
- `CHANGELOG.md`
- `docs/getting-started.md`
- `docs/release-install.md`
- `docs/release-packaging.md`
- `docs/release-versioning.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-11-release-download-installer-preflight.md`
- `docs/plans/2026-05-11-release-download-installer-execution-card.md`
- `docs/plans/2026-05-11-release-download-installer-closure.md`

## Forbidden Set

- WebUI.
- GitNexus-RC runtime/schema/Tool/WebUI.
- Live Cangjie/open-nwe repositories.
- Real Codex/opencode/Claude config files.
- New release tag or release asset mutation.

## Verification Target

- `bash -n scripts/*.sh`
- `bash scripts/check-release-metadata.sh`
- `bash scripts/install-release.sh --dry-run --version v0.1.0 --platform darwin-arm64`
- `bash scripts/install-release.sh --version v0.1.0 --platform darwin-arm64 --install-dir /tmp/codelattice-release-install-smoke-*`
- installed wrapper `--self-test`
- `cargo fmt --check`
- `git diff --check`
- `bash scripts/release-smoke.sh --tarball dist/codelattice-0.1.0-darwin-arm64.tar.gz`
- Tool detect-changes
- Tool index refresh
