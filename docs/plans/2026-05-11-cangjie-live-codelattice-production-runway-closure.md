# Cangjie Live CodeLattice Production Runway — Closure

**Date**: 2026-05-11
**Status**: Complete
**Commit**: (pending)

## Summary

Successfully established CodeLattice MCP as the production analysis entry for the live Cangjie codebase at `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui`.

## Results

### CodeLattice Analyze
- **3,046 nodes**, **7,693 edges**, **2,887 symbols**, **157 source files**
- All 6 quality gates: PASS (no synthetic nodes, no duplicates, no dangling edges)
- Analysis duration: ~11s (first run, uncached)
- Bridge JSON size: 2,975,658 bytes

### GitNexus-RC-Tool Registry
- New entry: **`cangjie-live-codelattice`**
  - 17,194 nodes, 52,522 edges, 197 clusters, 300 flows
  - Import via `--experimental-rust-core-bridge-graph` with `--name cangjie-live-codelattice`
- Old entries preserved:
  - `cjgui` (cangjie-GitNexus-Index): 6,333 symbols, 14,314 edges — stable fixture
  - `cjgui` (cangjie): 6,303 symbols, 11,157 edges — stale, not recommended

### MCP Tests (7/7 passed)
1. cache_prewarm — PASS
2. project_overview — PASS
3. graph_overview — PASS
4. symbol_search(init) — PASS
5. symbol_context(init) — PASS
6. production_assist — PASS
7. cache_status — PASS

### Tool Queries (verified)
- `context init -r cangjie-live-codelattice` → 20 candidates with disambiguation
- `detect-changes -r cangjie-live-codelattice --scope all` → 35 files, 3 symbols, risk low

## Architecture Clarification

| Layer | Role | Status |
|-------|------|--------|
| **CodeLattice** | Rust/Cangjie language intelligence core + MCP sidecar | Active, v0.8 |
| **GitNexus-RC-Tool** | Production CLI, registry, legacy query layer | Active |
| **GitNexus-RC** | Development workspace (not production) | Active |

## Naming Convention (Effective Now)

| Name | Path | Purpose |
|------|------|---------|
| `cangjie-live-codelattice` | `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui` | Live Cangjie analysis (recommended) |
| `cjgui-index` | `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui` | Stable test fixture |
| `cjgui` (legacy) | Both paths in Tool registry | Deprecated — do not use for new tasks |

## Files Changed

1. `crates/cli/src/mcp_server.rs` — Added `ALLOWED_DENIED_SUBPATHS` for `runtime/cjgui` exemption
2. `scripts/cangjie-live-codelattice-smoke.sh` — New: multi-mode smoke test for live cangjie
3. `docs/plans/2026-05-11-cangjie-live-codelattice-production-runway.md` — New: plan + identity doc
4. `docs/plans/2026-05-11-cangjie-live-codelattice-production-runway-closure.md` — This file
5. `docs/architecture/mcp-local-client-setup.md` — Updated safety notes, troubleshooting
6. `docs/architecture/mcp-v0-contract.md` — Updated changelog

## What Was NOT Changed

- GitNexus-RC runtime/schema/WebUI — untouched
- GitNexus-RC-Tool code/dist — untouched (only used Tool CLI for registry operations)
- `/Users/jiangxuanyang/Desktop/cangjie` source code — read-only, not modified
- No AGENTS.md/CLAUDE.md written to live repo
- Default tools not switched — GitNexus-RC MCP remains primary
- Old `cjgui` entries not removed — preserved for historical compatibility

## Next Steps

1. **When live repo stabilizes** (dirty count < 10): re-run `cangjie-live-codelattice-smoke.sh --full` for fresh graph
2. **When WebUI migration is ready**: update GitNexus-RC WebUI to prefer `cangjie-live-codelattice`
3. **When confidence is high**: consider switching default Cangjie tools from old `cjgui` to `cangjie-live-codelattice`
4. **When CodeLattice MCP is trusted as primary**: evaluate switching default tools entirely
