# Incremental Dirty-file Planning Pack — Preflight

Date: 2026-05-20
Status: Approved by user direction

## Goal

Extend the analysis scheduler from aggregate fingerprint reuse into dirty-file planning. When a cached entry is stale, CodeLattice should explain which project-local files changed and which analysis phases are affected, while still executing the existing full analysis path.

## Non-goals

- Do not implement partial graph rebuild execution in this pack.
- Do not add language semantics or parser features.
- Do not add a daemon, watcher, interpreter, or background scheduler.
- Do not execute target project build/test/package scripts.
- Do not modify GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool stable install, AI client configs, or real project source trees.
- Do not package or publish a release.

## Design Direction

- Keep `gitnexus-analysis-scheduler` as the owner of filesystem fingerprinting and phase planning.
- Add a compact file snapshot model that records path, size, mtime, and extension for scheduler-tracked files.
- Let `AnalysisRequest` optionally carry the previous snapshot from a cache entry.
- When a previous snapshot is present, compute an `incrementalPlan` with changed-file counts, a capped dirty-file list, affected phases, and a conservative execution strategy.
- Surface the plan in MCP cache metadata on cache miss/stale paths.
- Keep actual analysis execution full-scan for now and mark the plan as `planOnly: true`.

## Write Set

- `crates/analysis-scheduler/src/lib.rs`
- `crates/analysis-scheduler/tests/scheduler.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-20-incremental-dirty-file-planning-preflight.md`
- `docs/plans/2026-05-20-incremental-dirty-file-planning-closure.md`
- `docs/superpowers/specs/2026-05-20-incremental-dirty-file-planning-design.md`
- `docs/superpowers/plans/2026-05-20-incremental-dirty-file-planning.md`
- `docs/plans/README.md`
- `CHANGELOG.md`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool implementation
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- Codex/opencode/Claude config
- Real project source trees
- Target project build/test/package scripts

## Stop-line

Stop and report if this requires changing graph schema, changing language adapter output semantics, executing target project scripts, or if native impact review reports a concrete high/critical risk beyond the central MCP cache fanout already expected for this area.

## Verification Plan

- TDD RED/GREEN for scheduler dirty-file plan in `gitnexus-analysis-scheduler`.
- TDD RED/GREEN for MCP stale cache metadata exposing `incrementalPlan`.
- `cargo fmt --check`
- `git diff --check`
- `cargo test -p gitnexus-analysis-scheduler`
- targeted MCP scheduler/cache tests
- `cargo test --test mcp_server`
- `cargo test`
- `cargo test --all-features`
- `bash scripts/codelattice-mcp.sh --self-test`
- `bash scripts/mcp-dogfood.sh`
- `bash scripts/codelattice-precommit-check.sh`
- native detect-changes before commit

