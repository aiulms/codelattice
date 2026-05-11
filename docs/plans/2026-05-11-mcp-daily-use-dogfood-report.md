# MCP Daily-use Candidate Dogfood Report

> **日期：** 2026-05-11
> **版本：** v0.5.0

## Test Results

| Suite | Result |
|-------|--------|
| MCP tests (subprocess) | 48/48 pass (1 pre-existing flaky: unresolved_report) |
| Dogfood | 20/20 |
| Local client smoke | 9/9 |
| Cache smoke | 4/4 |
| Real client dry-run | 10/10 |
| Doctor | 5/5 |
| Workspace tests | All pass |
| Alpha trial (rust) | 5/5 |
| Alpha trial (cangjie) | 5/5 |

## New Behavior Verified

1. **mtime invalidation**: analyze → hit → (no mtime change) → still hit ✓
2. **LRU eviction**: cache_status shows maxEntries=16, totalEvictions tracked ✓
3. **Snippet in calls_from**: source candidates have sourceSnippet with lines ✓
4. **Snippet in rename_preview**: candidates have sourceSnippet ✓
5. **Production assist**: returns risk, recommendations, changed symbols ✓
6. **Compare runs**: bridge file diff with node/edge deltas ✓
7. **Doctor**: binary + wrapper + handshake + tools/list + cache_status ✓
8. **Real client dry-run**: 10 tool sequence all pass ✓
