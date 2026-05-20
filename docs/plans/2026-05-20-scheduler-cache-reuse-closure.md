# Scheduler Cache Reuse Pack — Closure

Date: 2026-05-20
Status: Completed

## Summary

- Scheduler fingerprints now participate in real MCP cache reuse decisions.
- Memory cache hits rebuild the current scheduler with the cached fingerprint and only return a hit when `decision.action == "reuse"`.
- Persistent cache entries now store `scheduler_fingerprint`; missing or mismatched scheduler fingerprints fail closed and trigger fresh analysis.
- Fresh analysis metadata can include `staleReason: "scheduler_fingerprint_changed"` when a memory cache entry was invalidated by scheduler mismatch.
- Existing source mtime, manifest hash, and docs mtime checks remain in place as compatibility guardrails.
- No scheduler fingerprint semantics changed, and no target project code is executed.

## Verification

- TDD RED: `cargo test --test mcp_server mcp_scheduler_fingerprint_ -- --nocapture` failed 0/2 because YAML config changes still returned cache hits.
- TDD GREEN: `cargo test --test mcp_server mcp_scheduler_fingerprint_ -- --nocapture` passed 2/2.
- `cargo test -p gitnexus-analysis-scheduler` passed 5/5.
- `cargo test --test mcp_server mcp_scheduler_ -- --nocapture` passed 4/4.
- `cargo test --test mcp_server mcp_persistent_cache_ -- --test-threads=1` passed 5/5.
- `cargo test --test mcp_server mcp_cache_ -- --test-threads=1` passed 15/15.
- `cargo test --test mcp_server mcp_cache_` passed 15/15 when run without concurrent persistent-cache mutation tests.
- A first parallel invocation of `mcp_cache_` and `mcp_persistent_cache_` was intentionally treated as invalid evidence because the persistent manifest test mutates the shared portable fixture while the cache tests use it.
- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo test --test mcp_server` passed 132/132 with default features.
- `cargo test` passed.
- `cargo test --all-features` passed, including 288/288 MCP all-feature tests.
- `bash scripts/codelattice-mcp.sh --self-test` passed with 50 tools.
- `bash scripts/mcp-dogfood.sh` passed 48/48.
- `bash scripts/codelattice-precommit-check.sh` passed. It reran `cargo fmt --check`, `git diff --check`, `cargo test --test productization_commands`, `cargo test --test mcp_server`, detect-changes smoke, debug build, and native detect-changes.

## Detect Changes Review

- Pre-edit impact preview reported `MEDIUM` for `McpCache::get_or_analyze`, `try_load_persistent`, `save_persistent`, and `PersistentCacheEntry`. No concrete high/critical symbol-level risk was reported.
- Final native detect-changes reported `critical` risk. Manual review attributes the risk to the central MCP cache entrypoint, broad workspace fanout, and two unsupported C# fixture boundary hits. The changed symbols are scoped to cache persistence/reuse metadata and tests, with no graph schema or language analyzer semantic changes.

## Boundaries

- Did not modify GitNexus-RC runtime/schema/WebUI.
- Did not modify GitNexus-RC-Tool.
- Did not modify `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Did not modify Codex/opencode/Claude config.
- Did not modify real project source trees.
- Did not execute target project build/test/package scripts.
