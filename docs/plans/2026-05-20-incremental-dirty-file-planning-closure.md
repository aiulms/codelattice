# Incremental Dirty-file Planning Pack — Closure

Date: 2026-05-20
Status: Implementation complete; native precommit passed

## Summary

- Scheduler now records a compact `FileSnapshot` for every scheduler-tracked file and can compare it with a previous snapshot.
- `AnalysisSchedule` now includes `incrementalPlan` with dirty-file counts, capped dirty-file details, affected phases, `planOnly`, and a conservative strategy.
- Source-only modified files are labeled `fileScopedCandidate`; structural/non-source changes such as YAML config are labeled `fullAnalysis`.
- MCP memory and persistent cache entries now store scheduler file snapshots and pass them back into fresh schedules when a cache entry becomes stale.
- MCP still executes the existing full analysis path on cache miss. This pack does not implement partial graph rebuild execution.

## Verification

- TDD RED: `cargo test -p gitnexus-analysis-scheduler dirty_file_plan -- --nocapture` failed because `with_previous_files`, `file_snapshot`, and `incremental_plan` did not exist.
- TDD GREEN: `cargo test -p gitnexus-analysis-scheduler dirty_file_plan -- --nocapture` passed.
- `cargo test -p gitnexus-analysis-scheduler` passed 6/6.
- TDD RED: `cargo test --test mcp_server mcp_scheduler_incremental_plan_ -- --nocapture` failed because MCP did not pass previous scheduler file snapshots.
- TDD GREEN: `cargo test --test mcp_server mcp_scheduler_incremental_plan_ -- --nocapture` passed.
- `cargo test --test mcp_server mcp_scheduler_ -- --nocapture` passed 5/5.
- `cargo test --test mcp_server mcp_cache_ -- --test-threads=1` passed 15/15.
- `cargo test --test mcp_server mcp_persistent_cache_ -- --test-threads=1` passed 5/5.
- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo test --test mcp_server` passed 133/133.
- `cargo test` passed.
- `cargo test --all-features` passed.
- `bash scripts/codelattice-mcp.sh --self-test` passed with 50 MCP tools and full language support flags.
- `bash scripts/mcp-dogfood.sh` passed 48/48 dogfood checks.
- `bash scripts/codelattice-precommit-check.sh` passed.

## Detect Changes Review

- Pre-edit native detect-changes reported `critical` from broad workspace fanout and unsupported C# fixture boundaries while only the preflight docs were untracked. No concrete high/critical changed symbol was present at that stage.
- Final native detect-changes reported `critical`: 6 tracked changed files, 4 untracked plan/spec files, 34 changed symbols, 14 affected projects, 11 workspace edges, and 2 unsupported C# fixture boundary hits.
- Review: the risk is expected for central scheduler and MCP cache changes. The concrete touched runtime path is the scheduler/cache metadata path, covered by scheduler unit tests, MCP scheduler/cache regressions, full MCP regression, full cargo tests, all-features tests, self-test, dogfood, and native precommit.
- The generic followup `no test files found in impact set` was reviewed; this pack does include new scheduler and MCP regression tests in the write set.

## Boundaries

- Did not modify GitNexus-RC runtime/schema/WebUI.
- Did not modify GitNexus-RC-Tool.
- Did not modify `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Did not modify Codex/opencode/Claude config.
- Did not modify real project source trees.
- Did not execute target project build/test/package scripts.
