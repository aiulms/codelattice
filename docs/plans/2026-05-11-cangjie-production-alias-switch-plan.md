# Cangjie Production Alias Switch Plan

**Date**: 2026-05-11
**Status**: Plan (no execution yet)
**Depends on**: CodeLattice v0.8 (`0035080`), cangjie-live-codelattice registry entry

## 1. Current Problem

### 1.1 Dual cjgui Ambiguity

GitNexus-RC-Tool registry has **two entries both named `cjgui`**:

| Alias | Path | Indexed | Commit | Symbols | Edges | Source |
|-------|------|---------|--------|---------|-------|--------|
| `cjgui` | `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index` | 2026-05-10 | `9b29db6` | 6,333 | 14,314 | GitNexus-RC-Tool native (stable checkout) |
| `cjgui` | `/Users/jiangxuanyang/Desktop/cangjie` | 2026-05-01 | `97bde56` | 6,303 | 11,157 | GitNexus-RC-Tool native (live dev, stale) |
| **`cangjie-live-codelattice`** | `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui` | 2026-05-11 | working tree | **17,194** | **52,522** | CodeLattice bridge (live working tree) |

When `-r cjgui` is used, Tool resolves to whichever entry it finds first — results are nondeterministic and often stale.

### 1.2 Stale Index → impact UNKNOWN/0

The live `cjgui` entry was indexed at commit `97bde56` (2026-05-01). The working tree is now at `b932bb8` with 114 dirty files. Any `detect-changes` or `impact` against this entry returns UNKNOWN because the graph doesn't cover the current state. This is correct Tool behavior, but the result is misleading if treated as "nothing to worry about."

### 1.3 Live vs Index Checkout

- **Live repo** (`/Users/jiangxuanyang/Desktop/cangjie`): Active development, HEAD `b932bb8`, 114 dirty. CodeLattice analyzed 3,046 nodes / 7,693 edges from `runtime/cjgui`. Tool imported 17,194 nodes / 52,522 edges from bridge JSON.
- **Index checkout** (`/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index`): Frozen mirror at `9b29db6`. Used for CI-like stable testing. Not the development tree.

## 2. Recommended Entries

| Name | Path | Purpose | Confidence |
|------|------|---------|------------|
| **`cangjie-live-codelattice`** | `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui` | Live production analysis (recommended) | High — fresh CodeLattice analysis, all 6 quality gates pass |
| `cjgui-index` | `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index` | Stable fixture for CI/testing | Medium — stable but limited to checkout snapshot |
| `cjgui` (legacy) | Both paths | **Deprecated** — do not use for new tasks | Low — ambiguous, stale |

## 3. Switch Phases

### Phase A: Docs/Scripts/Prompts Prohibit Bare `cjgui` (Current)
- ✅ `cangjie-live-codelattice-smoke.sh` uses explicit name
- ✅ `cangjie-production-alias-check.sh` uses explicit name
- ✅ Production runway doc defines naming convention
- ✅ This plan doc defines switch phases
- **Action**: All new CodeLattice docs/scripts/prompts must use `cangjie-live-codelattice` or `cjgui-index`, never bare `cjgui`

### Phase B: Execution AI Defaults to `cangjie-live-codelattice`
- **Trigger**: Live repo dirty count ≤ 10 (stable window green)
- **Action**: AI agent instructions prefer `cangjie-live-codelattice` for all Cangjie queries
- **Prerequisite**: At least one successful `--full` smoke with green stable window
- **Verification**: `context` / `detect-changes` / `impact` all return meaningful results

### Phase C: GitNexus-RC/WebUI/Tool UI Shows `cangjie-live-codelattice`
- **Trigger**: Phase B stable for ≥ 1 week
- **Action**: Update GitNexus-RC WebUI labels, Tool UI default selection
- **Note**: This phase requires GitNexus-RC changes — out of scope for CodeLattice-only work
- **Rollback**: Revert UI labels, keep CodeLattice sidecar unchanged

### Phase D: Hide/Cleanup Legacy `cjgui` Entries
- **Trigger**: Phase C stable for ≥ 2 weeks with no rollback
- **Action**: Consider `gitnexus remove` on stale `cjgui` entries
- **Safety**: Keep at least one `cjgui` entry as read-only historical reference
- **Prerequisite**: All consumers confirmed on `cangjie-live-codelattice`

## 4. Rollback Plan

1. **Preserve old `cjgui` entries** — never delete during Phase A-C
2. **CodeLattice sidecar doesn't replace GitNexus-RC MCP** — can always fall back
3. **If `cangjie-live-codelattice` fails**: revert to source code inspection / build / probe / forbidden scan — not bare `cjgui` impact
4. **Tool CLI has `remove` command** available for cleanup in Phase D only
5. **Stable window check** (`cangjie-production-alias-check.sh --status`) prevents accidental smoke during unstable periods

## 5. Explicit Non-Actions

- ❌ Do not modify GitNexus-RC runtime/schema/WebUI
- ❌ Do not modify GitNexus-RC-Tool code/dist
- ❌ Do not delete old registry entries (Phase D only, with confirmation)
- ❌ Do not switch default tools (GitNexus-RC MCP remains primary)
- ❌ Do not modify live cangjie source code
- ❌ Do not write AGENTS.md/CLAUDE.md to live repo

## 6. Current Stable Window Status

Updated 2026-05-11 (Phase B activation):
- Live repo dirty count: **10** → **GREEN** window ✅
- HEAD: `7759612`
- Phase B **activated**: [Phase B doc](2026-05-11-cangjie-phase-b-default-entry-switch.md)
- Agent command snippet: [snippet](2026-05-11-cangjie-live-agent-command-snippet.md)
- GREEN full smoke pass: analyze 3,073 nodes / 7,745 edges / 2,912 symbols, MCP 7/7, bridge 3MB, Tool ingest 17,377 symbols

Original status (2026-05-11, pre-GREEN):
- Live repo dirty count: **114** → **RED** window
- Phase B was blocked until dirty ≤ 10
