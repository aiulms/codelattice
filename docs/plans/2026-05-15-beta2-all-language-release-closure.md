# CodeLattice v0.13.0-beta.2 All-Language Release Closure

Date: 2026-05-15

## Result

`v0.13.0-beta.2` closes the external beta packaging drift found in `v0.13.0-beta.1`: the public package now builds the release binary with Cangjie, ArkTS, and TypeScript adapters enabled.

`v0.13.0-beta.1` remains immutable for checksum integrity. `v0.13.0-beta.2` is the replacement build for beta users who need the prebuilt macOS Apple Silicon package to analyze ArkTS / HarmonyOS or TypeScript projects.

## Root Cause

The beta.1 release documentation and CHANGELOG described ArkTS production-trial support, but `scripts/package-release.sh` built the published binary with only:

```bash
--features tree-sitter-cangjie
```

As a result, ArkTS auto-detection worked, but the packaged CLI rejected `language=arkts` with:

```text
ArkTS support is disabled. 请使用 --features tree-sitter-arkts 重新编译。
```

## Changes

- Bumped workspace product version to `0.13.0-beta.2` and refreshed `Cargo.lock`.
- Added MCP `initialize.serverInfo.arktsSupport` and `typescriptSupport` capability bits.
- Updated the release package build to use `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript`.
- Packaged ArkTS and TypeScript portable smoke fixtures alongside Rust and Cangjie fixtures.
- Strengthened release smoke to verify:
  - MCP tools/list exposes 22 tools.
  - `cangjieSupport`, `arktsSupport`, and `typescriptSupport` are all true in the unpacked release binary.
  - Rust, Cangjie, ArkTS, and TypeScript packaged fixtures analyze successfully.
- Updated local MCP wrapper, MCP installer, and local promotion script to prefer / build all-language binaries.
- Updated release docs, README status, install snippets, and smoke matrix from beta.1 to beta.2.

## Verification

Passed:

```bash
bash -n scripts/package-release.sh scripts/release-smoke.sh scripts/release-manifest.sh scripts/install-release.sh scripts/install-mcp.sh scripts/promote-to-local-tool.sh scripts/codelattice-mcp.sh
cargo fmt --check
git diff --check
cargo test
cargo test --all-features
bash scripts/check-release-metadata.sh
bash scripts/package-release.sh
bash scripts/release-smoke.sh --tarball dist/codelattice-0.13.0-beta.2-darwin-arm64.tar.gz
bash scripts/install-release.sh --dry-run --version v0.13.0-beta.2 --platform darwin-arm64
bash scripts/codelattice-mcp.sh --self-test
```

Release smoke confirmed:

```text
language support: OK cangjie=True arkts=True typescript=True
rust: OK symbols=9 files=2 edges=25
cangjie: OK symbols=22 files=3 edges=36
arkts: OK symbols=5 files=2 edges=24
typescript: OK symbols=20 files=4 edges=54
```

## Stop-Line Review

- No GitNexus-RC runtime, adapter, graph schema, or package changes.
- No production replacement behavior changed.
- No live repository source was modified.
- No macro expansion, full type inference, trait solving, or external crate resolution scope expansion.
- Release change is packaging / capability surfacing only; language analyzers keep their existing maturity boundaries.
