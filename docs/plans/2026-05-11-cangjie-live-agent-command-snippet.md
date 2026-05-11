# Cangjie Live Agent Command Snippet

**Effective**: 2026-05-11 (Phase B active)
**Copy-paste ready for any execution AI session**

---

## Default Entry

```
Repo name: cangjie-live-codelattice
Live root: /Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui
```

## Forbidden

- **Never use bare `cjgui`** — two entries share this name, results are ambiguous/stale
- **Never modify** `/Users/jiangxuanyang/Desktop/cangjie` source code

## Tool CLI

```bash
TOOL_CLI="/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"
```

Commands:
- `node $TOOL_CLI context <symbol> --repo cangjie-live-codelattice`
- `node $TOOL_CLI detect-changes --repo cangjie-live-codelattice --scope all`
- `node $TOOL_CLI impact <symbol> --repo cangjie-live-codelattice`
- `node $TOOL_CLI list`

## CodeLattice MCP

Root: `/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui`, Language: `cangjie`

Tools: `codelattice_project_overview`, `codelattice_symbol_search`, `codelattice_symbol_context`, `codelattice_production_assist`, `codelattice_analyze`, `codelattice_quality`, `codelattice_graph_overview`, `codelattice_calls_from`, `codelattice_calls_to`, `codelattice_impact_preview`

## Fallback Strategy

If `impact`/`context` returns UNKNOWN or 0 hits on `cangjie-live-codelattice`:
→ This does NOT mean "safe." Graph coverage may be incomplete.
→ Fallback to: source code inspection, build verification, probe, forbidden scan.
→ Do NOT fallback to bare `cjgui`.

## Fixture (testing only)

```
Path: /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui
Note: stable checkout, not live development tree
```

## Full Smoke Command

```bash
bash /Users/jiangxuanyang/Desktop/codelattice/scripts/cangjie-production-alias-check.sh --full
```
