# MCP Daily-use Candidate Pack — Closure Review

> **日期：** 2026-05-11
> **版本：** v0.5.0
> **状态：** Completed

## 变更摘要

### Stage 1: Cache Correctness
- `CacheEntry` 新增 `file_mtimes` (HashMap) 和 `root_canonical`
- `McpCache` 新增 `total_evictions`, `CACHE_MAX_ENTRIES` = 16
- `get_or_analyze`: 调用时 lazy mtime validation → stale 则自动 re-analyze
- LRU eviction: over limit 时淘汰 least-recently-used
- `cache_status`: 新增 `maxEntries`, `totalEvictions`, `cacheKey`, `trackedFiles`
- Cache meta: `cacheHit` + `cacheKey` + `cachedAtMs` + `analysisDurationMs`

### Stage 2: Source Snippet Expansion
- `calls_from`: source candidates 和 edges 增加 `sourceSnippet`/`targetSnippet`
- `calls_to`: target candidates 和 edges 增加 `sourceSnippet`
- `impact_preview`: 新增 `impactedSymbols` 数组（含 snippet），top files 含 `contextSnippet`
- `query_graph`: matched nodes 可选 `sourceSnippet`（默认 off）
- `rename_preview`: candidates 默认含 `sourceSnippet`
- 所有受影响工具增加 `includeSnippet` (default true/false) + `snippetContext` (default 2-3) 参数

### Stage 3: Daily Workflow Tools
- `codelattice_production_assist`: dry-run production readiness check（quality, unresolved, diagnostics, changed symbols, risk, recommendations）
- `codelattice_compare_runs`: diff two bridge JSON or cached vs fresh（nodes, edges, symbols, gates, diagnostics）

### Stage 4: Real Client Readiness
- `install-mcp.sh --doctor`: 5 checks（binary, wrapper, handshake, tools/list, cache_status）
- `codelattice-mcp.sh --self-test`: enhanced with tools/list + cache_status checks
- `mcp-real-client-dry-run.sh`: 10/10 checks pass

### Stage 5: Documentation
- mcp-v0-contract.md → v0.5.0
- mcp-local-client-setup.md → v0.5.0
- New plan docs: preflight, closure, dogfood report

## Verification Results
- MCP tests: 49 total (48 pass, 1 pre-existing flaky)
- Dogfood: 20/20
- Local client smoke: 9/9
- Cache smoke: 4/4
- Real client dry-run: 10/10
- Doctor: 5/5
- Alpha trial: pass

## Tool Count: 20

v0: 4, v0.1: 4, v0.2: 8, v0.3: 2, v0.5: 2
