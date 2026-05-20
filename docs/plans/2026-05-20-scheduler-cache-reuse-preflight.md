# Scheduler Cache Reuse Pack — Preflight

Date: 2026-05-20
Status: Approved for implementation

## Goal

Promote the analysis scheduler fingerprint from explanatory metadata into a real cache freshness signal. The cache should reuse analysis only when the scheduler says the current filesystem fingerprint matches the cached fingerprint.

## Non-goals

- Do not add language semantics.
- Do not replace tree-sitter parsers.
- Do not implement a daemon, watcher, parallel executor, or interpreter.
- Do not execute target project build/test/package scripts.
- Do not modify GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool stable install, AI client configs, or real project source trees.
- Do not do WebUI or release packaging work.

## Design Direction

Keep the existing two-layer cache and add scheduler-driven stale checks:

- Store the scheduler fingerprint with memory and persistent cache entries.
- On memory hit, rebuild the current scheduler with the cached fingerprint. Reuse only when `decision.action == "reuse"`.
- On persistent hit, compare the stored scheduler fingerprint before promoting the entry back into memory.
- Preserve existing mtime, manifest, and docs checks as compatibility/diagnostic guardrails.
- Surface `staleReason: "scheduler_fingerprint_changed"` when a memory entry is invalidated by the scheduler.

## Write Set

- `crates/analysis-scheduler/src/lib.rs`
- `crates/analysis-scheduler/tests/scheduler.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-20-scheduler-cache-reuse-preflight.md`
- `docs/plans/2026-05-20-scheduler-cache-reuse-closure.md`
- `docs/plans/README.md`
- `docs/superpowers/specs/2026-05-20-scheduler-cache-reuse-design.md`
- `docs/superpowers/plans/2026-05-20-scheduler-cache-reuse.md`
- `CHANGELOG.md`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- Codex/opencode/Claude config
- Real project source trees
- Target project build/test/package scripts

## Stop-line

Stop and report before implementation if impact analysis reports a concrete high/critical symbol-level risk in cache execution, if TDD cannot reproduce the current stale-cache behavior, or if the change requires altering language adapter output semantics.

## Verification Plan

- TDD RED/GREEN for scheduler fingerprint invalidating memory cache on a non-source file change.
- TDD RED/GREEN for scheduler fingerprint invalidating persistent cache on a non-source file change.
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

