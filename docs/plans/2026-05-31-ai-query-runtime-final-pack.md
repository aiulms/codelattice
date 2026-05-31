# AI Query Runtime Final Pack

Date: 2026-05-31

## Goal

Close the remaining runtime foundation gaps for AI-facing MCP usage:

1. **TypeScript project-once contract**: TypeScript jobs must use the in-process project-level analyzer, expose stage trace, and never warm facade cache through CLI fallback.
2. **Typed persistent GraphView snapshot**: persistent cache should store the query-ready GraphView indexes so a new MCP session can answer symbol/call/impact queries without rebuilding the graph view from full analyze JSON.
3. **Job warm persistence**: async job completion must warm both memory cache and persistent cache, so a job-created baseline survives MCP session restart.

The user goal is not merely "feature complete"; the tool should feel fast, concurrent, reliable, and token-efficient for AI agents. Fresh deltas remain first-class, while older baseline data can support cold start as long as freshness is explicit.

## Current State

- TypeScript already has `run_typescript_analysis_with_trace()` and job execution uses `run_project_analysis_once()`.
- MCP facade cache currently persists full `analyze_result` JSON, but persistent hits rebuild `GraphView::build()` every session.
- `McpCacheWarmer::warm_from_result()` inserts the job result into memory cache only; it does not save the newly warmed facade cache to the persistent layer.

## Execution Card

### Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-31-ai-query-runtime-final-pack.md`

### Forbidden Set

- Do not modify live repos such as `open-nwe`, `cangjie`, `warp`, or `openfang`.
- Do not modify `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not change MCP tool count or facade names.
- Do not introduce a watcher/daemon or broad graph schema rewrite in this pack.
- Do not add TypeScript type inference, full module semantic resolution, or runtime execution.

### Stop Lines

- If persistent cache compatibility requires invalidating all existing cache files, stop and choose a backward-compatible optional field instead.
- If the typed snapshot cannot be deserialized, fall back to rebuilding from `analyze_result`; never fail a query solely due to snapshot absence/corruption.
- If TypeScript analyzer falls back to CLI in a job warm path, treat it as a failing regression.

## Implementation Plan

1. Add failing tests:
   - persistent cache status marks entries with a typed GraphView snapshot.
   - persistent cache hit in a new session reports `graphViewCache = "persistent_typed_graph"`.
   - project job completion persists the typed GraphView snapshot for the next MCP session.
   - TypeScript job summary proves project-once/no CLI fallback warm path.
2. Add a serializable `PersistentGraphViewSnapshot` with the query indexes already used by `GraphView`.
3. Save the snapshot in `PersistentCacheEntry` as an optional field with `#[serde(default)]`.
4. Load persistent hits from the snapshot when available, attach `DocScanner`, and expose `graphViewCache` metadata.
5. Save persistent cache from `McpCacheWarmer::warm_from_result()` after job warm succeeds.
6. Keep old cache files readable by falling back to `GraphView::build(&analyze_result)`.

## Verification

Run focused tests first:

```bash
cargo test --test mcp_server mcp_persistent_cache_status_marks_typed_graph_snapshot -- --nocapture
cargo test --test mcp_server mcp_persistent_hit_uses_typed_graph_snapshot -- --nocapture
cargo test --test mcp_server mcp_project_job_persists_typed_graph_snapshot_for_next_session -- --nocapture
cargo test --test mcp_server mcp_typescript_job_uses_single_project_level_analysis --features tree-sitter-typescript -- --nocapture
```

Then run the closure gate:

```bash
cargo fmt --check
git diff --check
cargo test --test mcp_server
cargo test
scripts/codelattice-precommit-check.sh
```

Warn before commit if native precommit reports high or critical risk.
