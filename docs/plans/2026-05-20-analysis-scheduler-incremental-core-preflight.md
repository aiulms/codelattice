# Analysis Scheduler / Incremental Core Pack — Preflight

Date: 2026-05-20
Status: Approved for implementation

## Goal

Add a small bottom-layer analysis scheduler core that lets CodeLattice describe and reason about analysis work as deterministic jobs instead of ad hoc tool calls. This is a foundation pack for incremental analysis, cache reuse, and future daemon/watch mode.

## Non-goals

- Do not add new language semantics.
- Do not replace tree-sitter parsers.
- Do not execute target project build/test/package scripts.
- Do not build a runtime interpreter.
- Do not change GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool stable install, AI client configs, or real project source trees.
- Do not do WebUI work.

## Design Direction

This pack adds a parser-agnostic scheduler model:

- `AnalysisRequest`: normalized root/language/strict/scope input.
- `AnalysisFingerprint`: cheap filesystem fingerprint used for cache and staleness decisions.
- `AnalysisJobPlan`: ordered phases such as discover, parse, symbols, imports, calls, diagnostics, graph output.
- `AnalysisSchedule`: stable job envelope with reuse/fresh/stale decision metadata.

The initial implementation is intentionally read-only and synchronous. It does not parallelize real analysis yet; it creates the core contract and uses it to make cache status and prewarm output more explainable.

## Artifact Strategy

- Add an internal Rust crate for the scheduler core with unit tests.
- Reuse existing MCP cache and subprocess execution paths.
- Surface scheduler metadata through existing cache status/prewarm flows first, avoiding a broad new tool surface unless needed.

## Write Set

- `Cargo.toml`
- `crates/analysis-scheduler/Cargo.toml`
- `crates/analysis-scheduler/src/lib.rs`
- `crates/analysis-scheduler/tests/scheduler.rs`
- `crates/cli/Cargo.toml`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/mcp-dogfood.sh`
- `docs/plans/2026-05-20-analysis-scheduler-incremental-core-preflight.md`
- `docs/plans/2026-05-20-analysis-scheduler-incremental-core-closure.md`
- `docs/plans/README.md`
- `docs/superpowers/specs/2026-05-20-analysis-scheduler-incremental-core-design.md`
- `docs/superpowers/plans/2026-05-20-analysis-scheduler-incremental-core.md`
- `README.md`
- `CHANGELOG.md`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- Codex/opencode/Claude config
- Real project source trees
- Target project build/test/package scripts

## Stop-line

Stop and report before implementation if baseline tests fail, if GitNexus impact reports HIGH/CRITICAL for a symbol edit outside MCP cache/status plumbing, or if scheduler work requires changing language adapter semantics.

## Verification Plan

Baseline and final:

- `cargo fmt --check`
- `git diff --check`
- `cargo test -p gitnexus-analysis-scheduler`
- targeted MCP tests covering scheduler metadata
- `cargo test --test mcp_server`
- `cargo test`
- `cargo test --all-features`
- `bash scripts/codelattice-mcp.sh --self-test`
- `bash scripts/mcp-dogfood.sh`
- `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all`
