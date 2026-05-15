# CodeLattice Changelog

All notable CodeLattice release changes are tracked here.

This project follows the release policy in `docs/release-versioning.md`. The product version comes from Cargo `workspace.package.version`; MCP `serverVersion` is a separate runtime/tool-profile version.

## [Unreleased]

### Added

- (No unreleased changes yet.)

### Changed

- (No unreleased changes yet.)

### Fixed

- (No unreleased changes yet.)

## [0.13.0-beta.2] - 2026-05-15

### Added

- Release artifact now builds with `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript`, so the published macOS Apple Silicon tarball includes Rust, Cangjie, ArkTS, and TypeScript adapters.
- Packaged release smoke now includes ArkTS and TypeScript portable fixtures and verifies they analyze successfully from the unpacked tarball.
- MCP `initialize.serverInfo` now reports `arktsSupport` and `typescriptSupport` alongside `cangjieSupport`, making packaged language capability drift visible.

### Fixed

- Fixed `v0.13.0-beta.1` packaging drift where README/CHANGELOG advertised ArkTS production-trial support but the published binary was built without `tree-sitter-arkts`.

### Notes

- `v0.13.0-beta.1` remains immutable for checksum integrity. `v0.13.0-beta.2` supersedes it for external beta users who need ArkTS or TypeScript from the prebuilt package.

## [0.13.0-beta.1] - 2026-05-15

### Added

- `feat(mcp)`: add compact AI-sidecar outputs (`a7b1652`) - compact mode for MCP tools returns stripped-down results for AI context efficiency.
- `feat(arkts)`: complete production trial analysis path (`559f44a`) - ArkTS/HarmonyOS analysis works end-to-end via tree-sitter-typescript, component/buildMethod extraction.
- `feat(mcp)`: detect changed symbols from git diff (`9d0b157`) - auto-detect changed symbols from unstaged/staged/all git diff, map hunks to graph symbols.
- `feat(mcp)`: explain impact risk for AI review (`c674d19`) - impact preview returns riskReasons, impactMetrics, confidenceSummary, reviewFocus.
- `feat(mcp)`: associate code changes with docs (`7c19d41`) - static doc graph, DocScanner, code ↔ docs association for AI sidecar.
- `feat(typescript)`: add Phase A local graph support (`fb3719c`) - TypeScript language adapter, symbols, imports, calls.
- `feat(mcp)`: add persistent analysis cache (`c44b51d`) - two-layer cache (memory LRU + persistent disk), fingerprint stale detection, structured staleReasons.

### Changed

- (No breaking changes in this release cycle.)

### Documentation

- (Documentation updates tracked per feature above.)

### Known Limitations

- **TypeScript**: no path alias resolution, no monorepo/workspace support, no TSX framework hints.
- **ArkTS**: struct keyword parsed as ERROR by tree-sitter-typescript, no @Builder/@Extend, no full ArkUI declarative syntax tree.
- **Persistent cache**: no per-symbol incremental recompute.
- **Call edges** are heuristic with confidence/reason, not compiler-verified.
- **No project script execution**.
- **Not a compiler, IDE, language server, or hosted service**.

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
