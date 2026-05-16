# CodeLattice Homepage + Packaging Refresh Closure

Date: 2026-05-16
Working directory: `/Users/jiangxuanyang/Desktop/codelattice`

## Summary

Refreshed the GitCode-facing README/homepage and local release packaging metadata for the afternoon MCP diagnostic/workflow feature wave. The local beta candidate is now `0.15.0-beta.1`; the latest published GitCode Release page remains `v0.14.0-beta.1`.

No GitCode Release page, git tag, stable tool promotion, AI client config, GitNexus-RC, GitNexus-RC-Tool, or real project source was modified.

## Changes

- Bumped the Cargo workspace product version to `0.15.0-beta.1`.
- Updated README/homepage copy to describe CodeLattice as a local code intelligence engine with 37 MCP tools on current master.
- Moved the afternoon feature wave into `CHANGELOG.md` section `0.15.0-beta.1`.
- Updated packaging/getting-started/MCP docs and smoke matrix from 24-tool release assumptions to 37-tool current master assumptions.
- Updated packaging, release smoke, fresh clone smoke, install doctor, promote, Linux source smoke, and local client smoke scripts to expect 37 MCP tools.
- Fixed Python framework entry hint detection by normalizing graph node `name`/`kind`/`file`/`line` fields across top-level, `properties`, and Python id-derived forms.
- Fixed MCP `initialize.serverInfo.toolCount` to derive from `tools_list()` instead of a stale hardcoded count.

## Verification

Pre-commit verification run:

```bash
cargo fmt --check
git diff --check
bash scripts/check-release-metadata.sh
cargo test --test mcp_server
cargo test
cargo test --all-features
bash scripts/codelattice-mcp.sh --self-test
bash scripts/mcp-dogfood.sh
bash scripts/install-mcp.sh --doctor
bash scripts/package-release.sh
bash scripts/release-smoke.sh --tarball dist/codelattice-0.15.0-beta.1-darwin-arm64.tar.gz
```

Results:

- `cargo test --test mcp_server`: 114/114 passed.
- `cargo test`: passed.
- `cargo test --all-features`: passed, including 270/270 all-feature MCP tests.
- `scripts/codelattice-mcp.sh --self-test`: passed with 37 tools.
- `scripts/mcp-dogfood.sh`: 37/37 tools passed.
- `scripts/install-mcp.sh --doctor`: 8/8 passed.
- `scripts/release-smoke.sh`: passed across Rust, Cangjie, ArkTS, TypeScript, C, C++, and Python portable fixtures.

## Packaging Note

The final tarball must be regenerated after the commit so its manifest `sourceCommit` points at the committed closure state rather than the pre-commit HEAD. The post-commit artifact path remains:

`dist/codelattice-0.15.0-beta.1-darwin-arm64.tar.gz`

## Risks

- This is a beta candidate package, not GA.
- No core language analysis semantics were intentionally changed, but the MCP framework entry hint wrapper now handles Python graph node shapes that were previously skipped.
- `serverVersion` remains `0.13.0` by policy; product artifact version is `0.15.0-beta.1`.
