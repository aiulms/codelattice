# CodeLattice Homepage + Packaging Refresh Preflight

Date: 2026-05-16
Working directory: `/Users/jiangxuanyang/Desktop/codelattice`

## Goal

Refresh the GitCode-facing README and release packaging metadata after the afternoon diagnostic/workflow feature wave, then build and smoke a new local beta-candidate artifact.

## Current Truth

- HEAD at preflight: `4cc2e60`
- Latest public GitCode Release: `v0.14.0-beta.1` from commit `12d6373`, 24 MCP tools.
- Current master includes 37 MCP tools, including dead-code candidates, graph diagnostics, reachability, public API caution, framework entry hints, consistency/breaking-change/config review, workflow presets, and prompt cookbook docs.
- Worktree has pre-existing untracked `.claude/`; do not submit it.

## Version Strategy

Use product version `0.15.0-beta.1` for the new local package candidate. Rationale: the changes after `0.14.0-beta.1` are additive MCP tools and user-facing diagnostic workflows, which are larger than a patch/pre-release rebuild and must not reuse the already-published `0.14.0-beta.1` artifact identity.

Do not create a git tag or update the GitCode Release page in this task.

## Write Set

- `Cargo.toml`
- `README.md`
- `CHANGELOG.md`
- `docs/release/smoke-matrix.md`
- `docs/getting-started.md`
- `docs/release-packaging.md`
- release/package smoke scripts where tool count thresholds are stale
- this preflight and closure notes under `docs/plans/`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- CodeLattice-Tool stable runtime directory
- AI client configuration
- real project source code
- GitCode Release page or tag creation
- core language analysis semantics

## Impact Analysis

GitNexus impact was run on touched release scripts:

- `scripts/package-release.sh`: LOW, 0 impacted processes/modules.
- `scripts/release-smoke.sh`: LOW, 0 impacted processes/modules.
- `scripts/fresh-clone-smoke.sh`: LOW, 0 impacted processes/modules.

`check_tools_list` is not indexed as a target; script-level impact was used instead.

## Stop Line

Stop and report if:

- existing tests fail before packaging,
- package checksum/version cannot be verified,
- release smoke fails for the new artifact,
- GitCode push is rejected.

## Verification Plan

Run:

```bash
cargo fmt --check
git diff --check
bash scripts/check-release-metadata.sh
cargo test --test mcp_server
cargo test
cargo test --all-features
bash scripts/codelattice-mcp.sh --self-test
bash scripts/mcp-dogfood.sh
bash scripts/package-release.sh
bash scripts/release-smoke.sh --tarball dist/codelattice-0.15.0-beta.1-darwin-arm64.tar.gz
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
```

