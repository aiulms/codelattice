# CodeLattice Release Versioning + Changelog Execution Card

Date: 2026-05-11

## Write Set

- `CHANGELOG.md`
- `docs/release-versioning.md`
- `docs/release-packaging.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-11-release-versioning-changelog-preflight.md`
- `docs/plans/2026-05-11-release-versioning-changelog-execution-card.md`
- `docs/plans/2026-05-11-release-versioning-changelog-closure.md`
- `README.md`
- `scripts/check-release-metadata.sh`
- `scripts/package-release.sh`
- `scripts/release-smoke.sh`

## Forbidden Set

- GitNexus-RC runtime, schema, WebUI, or Tool code.
- Live Cangjie/open-nwe repositories.
- Real Codex, opencode, Claude, or shell configuration.
- Publishing assets or creating remote release objects.
- Renaming Cargo package or binaries.

## Verification Target

- `bash scripts/check-release-metadata.sh`
- `bash -n scripts/*.sh`
- `cargo fmt --check`
- `git diff --check`
- `bash scripts/package-release.sh`
- `bash scripts/release-smoke.sh`
- `bash scripts/fresh-clone-smoke.sh --skip-tests`
- `cargo test --test productization_commands`
- Tool detect-changes
- Tool index refresh if the changed docs/scripts warrant it
