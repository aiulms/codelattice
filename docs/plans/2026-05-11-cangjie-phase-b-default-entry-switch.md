# Phase B: Cangjie Default Live Entry Switch

**Date**: 2026-05-11
**Status**: Active
**Triggered by**: GREEN window full smoke pass (dirty=10, HEAD `7759612`)

## 1. Phase B Activation Conditions — All Met

| Condition | Status |
|-----------|--------|
| GREEN window full smoke pass | ✅ dirty=10, MCP 7/7, analyze 3,073 nodes |
| cangjie-live-codelattice registry refreshed | ✅ Indexed 2026-05-11 15:47, 17,377 symbols |
| context/detect-changes pass | ✅ context init → 20 candidates, detect-changes → 6 files low risk |
| All quality gates pass | ✅ 0 synthetic, 0 duplicate, 0 dangling |

## 2. Execution AI Default Rules

Effective immediately for all Cangjie tasks:

1. **Default repo name**: `cangjie-live-codelattice`
2. **Forbidden**: bare `cjgui` — do not use `-r cjgui` for any query
3. **Fixture/index checkout**: use descriptive name `cjgui-index` (not bare `cjgui`)
4. **UNKNOWN/0 is not safe**: if `impact` or `context` on `cangjie-live-codelattice` returns UNKNOWN or 0 hits, this does NOT mean "nothing to worry about." Must fallback to source code inspection / build verification / probe / forbidden scan.
5. **No live repo modification**: never modify `/Users/jiangxuanyang/Desktop/cangjie` source code

## 3. Recommended Command Templates

### Tool CLI (GitNexus-RC-Tool)

```bash
TOOL_CLI="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"

# Context lookup
node $TOOL_CLI context <symbol> --repo cangjie-live-codelattice

# Change detection
node $TOOL_CLI detect-changes --repo cangjie-live-codelattice --scope all

# Impact analysis
node $TOOL_CLI impact <symbol> --repo cangjie-live-codelattice
```

### CodeLattice MCP (via opencode or any MCP client)

```
codelattice_project_overview(root="/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui", language="cangjie")
codelattice_symbol_search(query="init", root="/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui", language="cangjie")
codelattice_production_assist(root="/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui", language="cangjie")
codelattice_symbol_context(name="init", root="/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui", language="cangjie")
```

### Full Smoke

```bash
bash scripts/cangjie-production-alias-check.sh --full
```

## 4. Rollback Rules

1. Do NOT delete old `cjgui` entries — they remain as legacy fallback
2. If `cangjie-live-codelattice` returns errors: fallback to source code inspection, build verification, probe — NOT bare `cjgui`
3. CodeLattice sidecar does not replace GitNexus-RC MCP
4. If live repo enters RED window (>50 dirty): revert to readonly analyze only

## 5. Explicit Non-Actions

- ❌ Do not switch GitNexus-RC WebUI
- ❌ Do not switch global default MCP
- ❌ Do not delete old registry entries
- ❌ Do not modify live cangjie source code
- ❌ Do not use bare `cjgui` for any production query

## 6. Phase Progression

- ✅ Phase A: Docs prohibit bare `cjgui` — complete
- ✅ **Phase B: AI defaults to `cangjie-live-codelattice`** — this document
- ⏳ Phase C: WebUI/Tool UI shows `cangjie-live-codelattice` — requires GitNexus-RC changes
- ⏳ Phase D: Hide/cleanup legacy `cjgui` — requires ≥ 2 weeks Phase C stability
