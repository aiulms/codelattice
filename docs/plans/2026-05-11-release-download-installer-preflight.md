# CodeLattice Release Download + Installer Preflight

Date: 2026-05-11

## Goal

Remove the stale README claim that the external release process is missing, and add a practical installer path for users who want to install the published GitCode Release without building from source.

## Scope

- Add `scripts/install-release.sh`.
- Add `docs/release-install.md`.
- Update README Quick Start with a GitCode Release install path.
- Update getting-started, release packaging, release versioning, changelog, and release metadata checks.

## Design

The installer is intentionally conservative:

- default release: `v0.1.0`
- default install dir: `$HOME/.local/share/codelattice-tool`
- platform auto-detected from `uname`
- release URL shape: `https://gitcode.com/aiulms/codelattice/releases/download/<tag>/<artifact>`
- verifies `.sha256` before unpacking
- validates unpacked and installed `codelattice-mcp.sh --self-test`
- refuses to overwrite a non-empty non-CodeLattice directory unless `--force` is passed
- never writes AI client configuration

The current public artifact is `darwin-arm64`. Linux and other platforms remain source-build paths until multi-platform artifacts are published.

## Non-goals

- No WebUI.
- No package manager integration.
- No automatic AI client config writes.
- No new release tag.
- No change to Rust/Cangjie analysis semantics.
- No GitNexus-RC / Tool / live repo changes.

## Risk

Risk is low. Changes are shell installer and docs. Installer writes only the requested install directory and temporary download directory.
