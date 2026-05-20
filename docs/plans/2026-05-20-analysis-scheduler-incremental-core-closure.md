# Analysis Scheduler / Incremental Core Pack — Closure

Date: 2026-05-20
Status: Completed

## Summary

- Added internal crate `gitnexus-analysis-scheduler`.
- Added deterministic analysis phases:
  - discover
  - fingerprint
  - parse
  - symbols
  - imports
  - calls
  - diagnostics
  - graph
- Added cheap filesystem fingerprinting with hidden/generated directory exclusion.
- Wired scheduler metadata into existing MCP cache flows:
  - `codelattice_cache_prewarm` returns `schedule`
  - `codelattice_cache_status` memory entries include `scheduler`
- Tightened `scripts/mcp-dogfood.sh` for the `codelattice_workflow` contract after verification showed the script still expected the older generic facade envelope.
- This is a foundation layer only. It does not execute project code, replace tree-sitter, add language semantics, or introduce a runtime interpreter.

## Verification

- TDD RED: `cargo test -p gitnexus-analysis-scheduler` failed before the crate API existed.
- TDD GREEN: `cargo test -p gitnexus-analysis-scheduler` passed 5/5.
- MCP RED: `cargo test --test mcp_server mcp_scheduler_ -- --nocapture` failed because scheduler metadata was absent.
- MCP GREEN: `cargo test --test mcp_server mcp_scheduler_ -- --nocapture` passed 2/2.
- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo test --test mcp_server` passed 130/130.
- `cargo test` passed.
- `cargo test --all-features` passed, including 286 MCP tests under full language features.
- `bash scripts/codelattice-mcp.sh --self-test` passed with 50 tools and all language support flags enabled.
- `bash scripts/mcp-dogfood.sh` passed 48/48 after the workflow facade assertion was aligned to `ai.workflow.v1`.
- `bash scripts/codelattice-mcp-facade-smoke.sh` passed 13/13 when run standalone. A first parallel run returned empty JSON during concurrent dogfood execution and was treated as a script/runtime race, not a product failure.
- `bash scripts/codelattice-precommit-check.sh --full` passed.

## Detect Changes Review

Native detect-changes reported `critical` at the workspace level:

- Changed Rust symbols were individually `LOW`: `CacheEntry`, `build_scheduler_metadata`, `scheduler_fingerprint`, `McpCache::get_or_analyze`, `McpCache::insert_memory_entry`, and `McpCache::status`.
- The overall `critical` rating came from broad workspace propagation, a script change, and two unsupported C# fixture boundary hits.
- The warning was reviewed before commit. The implementation remains scoped to the scheduler crate, MCP cache metadata plumbing, tests, docs, and the dogfood assertion fix.

## Boundaries

- Did not modify GitNexus-RC runtime/schema/WebUI.
- Did not modify GitNexus-RC-Tool.
- Did not modify `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Did not modify Codex/opencode/Claude config.
- Did not modify real project source trees.
