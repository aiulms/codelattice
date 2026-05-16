# Framework Entry Hints — Implementation Closure

Following preflight card `docs/plans/2026-05-16-framework-entry-hints-preflight.md`.

## Key Implementation Decisions

1. **New tool name**: `codelattice_framework_entry_hints`
2. **Tool index**: #33 (from 32 → 33)
3. **Handler**: follows same pattern as `compute_external_api_surface` — graph-only, no re-analysis
4. **Fixture pattern**: mirrors external-api-surface (10 files) but focused on framework patterns
5. **Integration approach**: lazy — each existing tool optionally queries `compute_framework_entry_hints` for symbols in its scope
6. **Score threshold**: 0.35 minimum, same as external_api_surface

## Handler functions to implement
- `compute_framework_entry_hints(gv, language, options) -> Value`
- `detect_framework_entry_hints(gv, options) -> Vec<FrameworkEntryHint>`
- `score_framework_entry_hint(symbol, file, reasons, gv, options) -> f64`
- `build_framework_entry_cautions() -> Vec<String>`
- `build_framework_entry_verification() -> Vec<String>`
- `handle_framework_entry_hints(cache, params) -> Result<Value, Value>`

## Integration functions to modify
- `compute_reachability_map()` — add framework hints to summary + entryPoints
- `compute_dead_code_candidates()` — check symbols against framework hints
- `compute_external_api_surface()` — add component/route signal (minor)
- `compute_review_plan()` — add checklist items for framework symbols
- `compute_project_insights()` — prioritize route/handler files

## Test plan (10 tests)
1. `mcp_framework_entry_hints_python_routes` — get_user/createOrder detected
2. `mcp_framework_entry_hints_python_cli` — sync_command detected
3. `mcp_framework_entry_hints_typescript_routes` — getUser/createOrder detected
4. `mcp_framework_entry_hints_typescript_component` — UserCard detected
5. `mcp_framework_entry_hints_compact_shape` — compact=true omits verbose data
6. `mcp_reachability_map_includes_framework_hints` — summary has frameworkEntryHintCount
7. `mcp_dead_code_candidates_framework_caution` — route handler not high-conf dead code
8. `mcp_review_plan_framework_checklist` — checklist mentions framework
9. `mcp_framework_entry_hints_no_runtime_proof` — generatedFrom.runtimeVerified=false
10. `mcp_framework_entry_hints_auto_language` — auto-detect works

## Feature gates
- Python tests: `#[cfg(feature = "tree-sitter-python")]`
- TypeScript tests: `#[cfg(feature = "tree-sitter-typescript")]`
