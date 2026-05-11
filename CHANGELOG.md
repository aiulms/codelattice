# CodeLattice Changelog

All notable CodeLattice release changes are tracked here.

This project follows the release policy in `docs/release-versioning.md`. The product version comes from Cargo `workspace.package.version`; MCP `serverVersion` is a separate runtime/tool-profile version.

## [Unreleased]

### Added

- No unreleased entries yet.

## [0.1.0] - 2026-05-11

### Added

- Public `codelattice` release binary, while retaining `gitnexus-rust-core-cli` as a compatibility binary.
- Portable release tarball packaging with `manifest.json`, stable MCP wrapper, checksums, docs, and Rust/Cangjie smoke fixtures.
- Release smoke validation for packaged binaries, wrapper self-test, MCP `tools/list`, and portable Rust/Cangjie fixture analysis.
- Fresh clone smoke workflow for external-user setup validation without writing AI client configuration files.
- Portable MCP install/promote scripts with configurable install directories.

### Changed

- README and getting-started docs now present CodeLattice as a standalone local code intelligence engine for Rust and Cangjie projects.
- MCP setup docs and generated config snippets now prefer stable promoted runtime paths over developer checkout wrappers.

### Fixed

- Cangjie `project_overview` compact output now reports nonzero top-level symbol, source file, and edge counts for populated projects.
- Install and promote scripts no longer assume the original author's machine path.

### Compatibility

- The Cargo package and compatibility binary name `gitnexus-rust-core-cli` remain available for existing scripts.
- The public command surface should prefer `codelattice` for new usage.
