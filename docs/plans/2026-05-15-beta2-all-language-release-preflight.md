# CodeLattice v0.13.0-beta.2 All-Language Release Preflight

Date: 2026-05-15

## Trigger

External beta user installed the `v0.13.0-beta.1` release tarball and analyzed an ArkTS / HarmonyOS project. Auto-detection selected `language=arkts`, but the packaged binary returned:

```text
ArkTS support is disabled. 请使用 --features tree-sitter-arkts 重新编译。
```

## Root Cause

The release tarball was built by `scripts/package-release.sh` with only:

```text
--features tree-sitter-cangjie
```

The source tree contains ArkTS and TypeScript adapters, and README/CHANGELOG describe ArkTS as production trial and TypeScript as Phase A, but the published binary artifact did not include those feature flags.

## Impact Check

GitNexus pre-change impact:

- `scripts/package-release.sh`: LOW, no upstream execution flows.
- `scripts/release-smoke.sh`: LOW, no upstream execution flows.
- `scripts/install-release.sh`: LOW, no upstream execution flows.
- root `Cargo.toml`: LOW, no upstream execution flows.
- `crates/cli/Cargo.toml`: LOW, no upstream execution flows.
- `handle_request` in `crates/cli/src/mcp_server.rs`: LOW; direct caller `run_mcp_server`.

## Write Set

- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- `README.md`
- `docs/README.en.md`
- `docs/getting-started.md`
- `docs/release-install.md`
- `docs/release-packaging.md`
- `docs/release/smoke-matrix.md`
- `docs/platforms/linux-openeuler.md`
- `scripts/install-release.sh`
- `scripts/package-release.sh`
- `scripts/release-manifest.sh`
- `scripts/release-smoke.sh`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`

## Forbidden Set

- Do not modify runtime language analysis semantics except MCP initialize metadata.
- Do not change GitNexus-RC runtime/schema/package.
- Do not modify external live repos.
- Do not overwrite existing `v0.13.0-beta.1` release artifacts or checksum.
- Do not run destructive git commands.

## Execution Card

1. Add failing coverage:
   - MCP initialize must expose `arktsSupport` and `typescriptSupport`.
   - Release smoke must verify packaged ArkTS/TypeScript fixture analysis when support is enabled.
2. Bump product version to `0.13.0-beta.2`.
3. Build release binaries with `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript`.
4. Package ArkTS and TypeScript portable fixtures.
5. Extend manifest and wrapper self-test with ArkTS/TypeScript support fields.
6. Update docs and release notes so beta.2 is the default install target.
7. Verify:
   - `cargo fmt --check`
   - `git diff --check`
   - `cargo test`
   - `cargo test --all-features`
   - `bash scripts/check-release-metadata.sh`
   - `bash scripts/package-release.sh`
   - `bash scripts/release-smoke.sh --tarball dist/codelattice-0.13.0-beta.2-darwin-arm64.tar.gz`
   - `bash scripts/install-release.sh --dry-run --version v0.13.0-beta.2 --platform darwin-arm64`
8. Commit, tag `v0.13.0-beta.2`, push `master` and tag to GitCode.
9. Create GitCode release page and upload `.tar.gz` plus `.sha256`.

## Stop-Line

If all-feature build fails because ArkTS or TypeScript dependencies are not portable on the release host, stop before tagging and publish only a source-build advisory instead.
