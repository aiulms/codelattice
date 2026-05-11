# CodeLattice Release Versioning

CodeLattice release metadata has two separate version tracks:

- Product version: Cargo `workspace.package.version`.
- MCP profile version: MCP serverVersion in runtime manifests and MCP `initialize` responses.

The product version is what users see in release artifacts and `codelattice --version`. The MCP serverVersion describes the sidecar tool/profile contract exposed over MCP. They may advance at different speeds and must not be treated as interchangeable.

## Source of Truth

`Cargo.toml` `[workspace.package] version` is the source of truth for CodeLattice product releases.

It controls:

- `codelattice --version`
- the default `scripts/package-release.sh` artifact name
- the required version section in `CHANGELOG.md`

Release artifacts use this shape:

```text
codelattice-<version>-<platform>.tar.gz
codelattice-<version>-<platform>.tar.gz.sha256
```

## SemVer Rules

Use patch versions for:

- compatible bug fixes
- documentation updates
- packaging, install, and smoke-script fixes
- internal refactors with no CLI, JSON, MCP, or quality-gate contract break

Use minor versions for:

- additive CLI commands or flags
- additive MCP tools
- additive JSON fields
- compatible quality gates
- new supported fixtures, language coverage, or release artifact contents

Use major versions for:

- removed or renamed CLI commands, flags, or exit-code meanings
- removed or renamed stable JSON/MCP fields
- incompatible quality-gate semantics
- removal of the compatibility binary before an announced deprecation window ends
- Rust toolchain or platform support changes that break existing supported users

## Changelog Rules

`CHANGELOG.md` must contain:

- `## [Unreleased]`
- one dated section for the current product version, for example `## [0.1.0] - 2026-05-11`
- entries grouped under the categories below when relevant

Allowed categories:

- Added
- Changed
- Fixed
- Deprecated
- Removed
- Security
- Compatibility
- Internal

Every release version must have at least one meaningful changelog entry. Do not publish an empty version section.

## Release Checklist

Before publishing or sharing an artifact:

```bash
bash scripts/check-release-metadata.sh
cargo fmt --check
git diff --check
cargo test
cargo test --features tree-sitter-cangjie
bash scripts/package-release.sh
bash scripts/release-smoke.sh
bash scripts/fresh-clone-smoke.sh --skip-tests
```

For a version bump:

1. Update Cargo `workspace.package.version`.
2. Move relevant `Unreleased` entries into `## [<version>] - <date>`.
3. Keep a fresh `## [Unreleased]` section at the top.
4. Run `bash scripts/check-release-metadata.sh`.
5. Build and smoke the release tarball.
6. Tag the release as `v<version>` only after the artifact passes smoke.

## Compatibility Naming

`codelattice` is the primary public binary. `gitnexus-rust-core-cli` remains a compatibility binary for existing scripts and should be removed only through an announced deprecation path in a future major release.
