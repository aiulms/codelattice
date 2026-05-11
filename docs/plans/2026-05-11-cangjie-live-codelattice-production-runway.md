# Cangjie Live CodeLattice Production Runway

**Date**: 2026-05-11
**Status**: Complete
**Scope**: Establish CodeLattice as the definitive production analysis entry for the live Cangjie codebase
**Follow-up**: [Production Alias Switch Plan](2026-05-11-cangjie-production-alias-switch-plan.md)

## Problem Statement

GitNexus-RC-Tool registry currently has **two repos both named `cjgui`**:

| Registry Name | Path | Source | Last Indexed |
|---|---|---|---|
| `cjgui` | `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index` | GitNexus-RC-Tool native indexing of a clean checkout | 2026-05-10 |
| `cjgui` | `/Users/jiangxuanyang/Desktop/cangjie` | GitNexus-RC-Tool native indexing of live dev workspace | 2026-05-01 |

### Why bare `cjgui` is unreliable

1. **Name collision**: Two different repos share the name `cjgui`. Tool queries using `-r cjgui` are ambiguous — Tool falls back to path-based disambiguation, but the result depends on registry order, not recency or relevance.

2. **Live repo dirty state**: `/Users/jiangxuanyang/Desktop/cangjie` has 104 dirty files at HEAD `b932bb8`. The Tool index from 2026-05-01 is stale — it reflects a commit ~8 days and 3+ commits behind current HEAD. Any `impact` or `detect-changes` result against this entry is **UNKNOWN/0** because the indexed graph doesn't match the working tree.

3. **Index checkout vs live**: The `cangjie-GitNexus-Index` checkout at HEAD `9b29db6` is a **frozen mirror** used for stable CI-like testing. It's not the live development tree. It has 380 files / 6,333 symbols / 14,314 edges — a subset of the live repo's actual content.

### Why `impact UNKNOWN/0` is the expected result

When Tool registry has a stale index (indexed at an old commit) and the working tree has since diverged significantly:
- `detect-changes` maps git diff hunks to indexed symbols → most hunks land in files not in the index
- `impact` walks the graph from changed symbols → no edges found → returns `UNKNOWN` or `0`
- This is **correct behavior** — the graph simply doesn't cover the current state

## Identity Convention

Effective immediately, all new CodeLattice and Tool operations use these names:

| Name | Path | Purpose | Analysis Engine |
|---|---|---|---|
| `cangjie-live-codelattice` | `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui` | Live Cangjie production analysis via CodeLattice | CodeLattice MCP (Rust/Cangjie tree-sitter) |
| `cjgui-index` | `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui` | Stable fixture for CI/testing | CodeLattice MCP (Rust/Cangjie tree-sitter) |
| `cjgui` (legacy) | Both paths in Tool registry | Historical only — do not use for new tasks | GitNexus-RC-Tool native |

### Rules

1. **New tasks always use `cangjie-live-codelattice`** for live repo analysis
2. **Test fixtures use `cjgui-index`** or explicit path references
3. **Never rely on bare `cjgui`** for any production or development decision
4. **`cangjie-live-codelattice` is read-only** — CodeLattice analyzes but never modifies cangjie source
5. **No AGENTS.md/CLAUDE.md** written to live repo — all artifacts go to `/tmp/codelattice-cangjie-live-*`

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    AI Client (opencode)                   │
│         queries CodeLattice MCP via stdio                │
└───────────────────────┬─────────────────────────────────┘
                        │ MCP (JSON-RPC over stdio)
                        ▼
┌─────────────────────────────────────────────────────────┐
│              CodeLattice MCP (v0.7+)                      │
│   ┌─────────────────┐  ┌──────────────────────────────┐ │
│   │ Rust Analyzer    │  │ Cangjie Analyzer              │ │
│   │ (tree-sitter)    │  │ (tree-sitter-cangjie)         │ │
│   └─────────────────┘  └──────────────────────────────┘ │
│   21 tools: analyze, quality, symbol_search, etc.        │
│   Profile: cangjieSupport=True, toolCount=21             │
└───────────┬──────────────────────┬───────────────────────┘
            │                      │
            ▼                      ▼
   /Users/.../codelattice   /Users/.../cangjie/runtime/cjgui
   (self-analysis)          (cangjie-live-codelattice)
            │
            │ bridge JSON export
            ▼
┌─────────────────────────────────────────────────────────┐
│           GitNexus-RC-Tool Registry                      │
│   codelattice        → CodeLattice self-index            │
│   cjgui (legacy ×2)  → old indexes, not for new use     │
│   cangjie-live-codelattice → NEW, from CodeLattice bridge │
└─────────────────────────────────────────────────────────┘
```

## Current Limitations

1. **WebUI not updated** — GitNexus-RC WebUI still shows old `cjgui` entries
2. **Default tools not switched** — opencode still uses GitNexus-RC MCP as primary
3. **Tool `cjgui` entries remain** — not deleted, just deprecated for new use
4. **Live repo dirty state** — 104 dirty files means analysis reflects working tree, not HEAD commit

## Implementation Stages

- [x] Stage 0: Truth Gate — verify clean state, run existing tests
- [ ] Stage 1: Identity Cleanup (this doc)
- [ ] Stage 2: Live Readonly Analysis Script
- [ ] Stage 3: Live Quality Checks
- [ ] Stage 4: Tool Registry Non-Destructive Ingestion
- [ ] Stage 5: MCP/CLI Production Readiness Docs
- [ ] Stage 6: Full Verification
- [ ] Stage 7: Tool Index Refresh
- [ ] Stage 8: Commit + Push

## Next Steps (Beyond This Runway)

1. When live repo stabilizes (dirty count < 10), re-index `cangjie-live-codelattice` for fresh graph
2. Consider removing old `cjgui` entries from Tool registry after migration confirmed
3. Evaluate switching WebUI to display CodeLattice-sourced graphs
4. Evaluate switching default tools from GitNexus-RC MCP to CodeLattice MCP for Cangjie projects
