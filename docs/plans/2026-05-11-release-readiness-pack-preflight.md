# CodeLattice Release Readiness Pack Preflight（2026-05-11）

## Goal

在不做 WebUI 的前提下，把 CodeLattice 推进到可对外发布的非 UI 产品化状态：

- release tarball packaging
- tarball-level release smoke
- `codelattice` binary alias
- public getting-started / release packaging docs
- README 优先展示 `codelattice` 命令，同时保留旧 binary 兼容说明

## Scope

Writes allowed in this round：

- `Cargo.toml`
- `.gitignore`
- `crates/cli/Cargo.toml`
- `crates/cli/src/`
- `crates/cli/tests/productization_commands.rs`
- `scripts/build.sh`
- `scripts/install-mcp.sh`
- `scripts/promote-to-local-tool.sh`
- `scripts/codelattice-mcp.sh`
- `scripts/smoke.sh`
- `scripts/verify-bridge.sh`
- `scripts/alpha-trial-smoke.sh`
- `scripts/mcp-dogfood.sh`
- `scripts/mcp-real-client-dry-run.sh`
- `scripts/mcp-cache-smoke.sh`
- `scripts/package-release.sh`
- `scripts/release-smoke.sh`
- `README.md`
- `docs/getting-started.md`
- `docs/release-packaging.md`
- `docs/architecture/`
- `docs/plans/`

Forbidden：

- No WebUI
- No GitNexus-RC runtime/schema/WebUI changes
- No GitNexus-RC-Tool changes
- No live Cangjie repo / open-nwe changes
- No real Codex/opencode/Claude config writes
- No behavior change to Rust/Cangjie analysis semantics
- No dependency additions unless a verification blocker proves unavoidable

## Baseline

Repo：`/Users/jiangxuanyang/Desktop/codelattice`

HEAD before this round：`7e31924 docs(readme): reposition codelattice homepage`

Working tree before this round：

- tracked files clean
- untracked private dirs: `.agents/`, `.claude/`

Baseline commands：

- `cargo fmt --check` — PASS
- `git diff --check` — PASS
- `cargo test --test productization_commands` — PASS（11/11）

Known warnings kept out of scope：

- `run_subcommand_with_timeout` unused `timeout`
- `build_graph_view` unused

## Impact Analysis

GitNexus CLI：

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js impact CodeLattice --repo codelattice --direction upstream
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js impact 'Function:crates/cli/src/main.rs:main' --repo codelattice --direction upstream
```

Results：

- README `CodeLattice` section: LOW, 0 affected processes
- CLI `main`: LOW, 0 affected processes

Expected changed runtime surface is Cargo binary metadata only: both `codelattice` and `gitnexus-rust-core-cli` should execute the same `crates/cli/src/main.rs`.

## Risk

Primary risks：

1. `codelattice` alias may not build or may not be visible to `assert_cmd`.
2. Release tarball may accidentally depend on dev checkout paths.
3. Release smoke may mutate real client config or stable runtime.
4. README may overstate packaging status before tests verify it.

Mitigations：

- TDD: add failing `Command::cargo_bin("codelattice")` test before Cargo alias.
- TDD: add `scripts/release-smoke.sh` before package script and verify it fails because packaging is absent.
- Release smoke must use `/tmp` temp dirs and never write client config.
- Tarball smoke must invoke extracted binary and wrapper from extracted artifact only.

## Stop-line

This pack stops at release-readiness artifacts. It does not:

- rename package IDs fully
- publish to crates.io
- create GitCode release assets
- implement WebUI
- implement HTTP daemon/server mode
- change MCP schema
- change analysis behavior
