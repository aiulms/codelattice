# Entry Point & Reachability Pack — Preflight

**Date**: 2026-05-16
**Status**: Pre-implementation
**HEAD**: `d01e631`

## Goal

Add `codelattice_reachability_map` MCP tool with multi-language entry point detection and static graph reachability analysis. Integrate results into `dead_code_candidates`, `project_insights`, `review_plan`, and optionally `production_assist`.

## Definitions

### Entry Point Candidate
A symbol or file identified by static heuristic as a likely program entry point. **NOT a runtime proof** — the symbol may not actually be the runtime entry, and there may be entries the heuristic misses.

### Reachable Symbol
A symbol reachable from entry point candidates by traversing graph edges (CALLS, IMPORTS, REFERENCES, INCLUDES, DEFINES) via BFS. **NOT runtime reachability** — dynamic dispatch, reflection, plugin systems, and external consumers may create additional runtime paths.

### Unreachable Candidate
A symbol NOT reachable from any detected entry point candidate. Still may be used by:
- External consumers (public API)
- Dynamic dispatch (trait objects, reflection, plugin systems)
- Tests (when tests excluded from analysis)
- Framework magic (decorators, annotations, convention-based routing)

## Existing Infrastructure

The following functions already exist in `mcp_server.rs`:

- `detect_entry_like(name, kind, file, language, fan_out)` → bool (L6829)
  - Language-specific heuristics for Rust, Cangjie, ArkTS, TypeScript, generic
- `detect_entry_points(gv, language, entry_hints)` → Vec<(id, name, kind, file, line)> (L8089)
  - Full entry point detection with file suffix matching + user hints
- `reachable_from_entry_points(gv, entry_points)` → HashSet<String> (L8172)
  - BFS traversal, max_depth=8, follows CALLS/REFERENCES/IMPORTS/INCLUDES/DEFINES
- `is_generated_path(file)` → bool (L8209)
- `has_dynamic_pattern(name, file)` → bool (L8221)
- `is_test_like_path(file)` → bool (L8237)
- `score_candidate_symbols(...)` (L8307) — used by dead_code_candidates
- `score_candidate_files(...)` (L8533) — used by dead_code_candidates

## Changes Required

### 1. Enhanced Entry Point Detection

Extend `detect_entry_like()` and `detect_entry_points()` with:
- **Python**: `__main__.py`, `if __name__ == "__main__"` detection (if possible via symbol props), `create_app`, route decorator hints
- **C**: `WinMain` detection, exported header functions with caution
- **C++**: `WinMain`, `wWinMain`, `DllMain`, constructor attributes, exported header API with caution
- **Cangjie**: public package-level symbols with caution
- All: confidence scoring (high/medium/low) instead of just bool

### 2. New: `compute_reachability_map()`

Standalone MCP tool with rich output:
- Entry point candidates with confidence + reasons
- Reachable symbols/files with traversal metadata
- Unreachable candidates with cautions
- Summary counts
- `generatedFrom` with `runtimeVerified: false`

Parameters: `root`, `language`, `compact`, `limit`, `maxDepth`, `includeTests`, `includePublicApi`, `includeReachableItems`, `entryHints`, `excludePatterns`

### 3. Integration into Existing Tools

- `dead_code_candidates`: Add `entryPointCount`, `reachableSymbolCount`, `not-reachable-from-entry-points` reason, `runtimeVerified: false`
- `project_insights`: Use enhanced entry detection, add entry points to readFirst
- `review_plan(mode=onboarding)`: Add "start from these entry points"
- `review_plan(mode=release_check)`: Add reachability summary
- `production_assist`: Optional — warn if changed symbol is unreachable but public API

### 4. New Fixture

`fixtures/typescript/reachability-map/` with:
- `tsconfig.json`
- `src/index.ts` — imports app, starts it
- `src/app.ts` — imports routes/live
- `src/routes.ts` — routing
- `src/live.ts` — reachable live function
- `src/legacy.ts` — unused oldHelper (unreachable)
- `src/public-api.ts` — exports publicUtility (not used internally, public API caution)
- `src/dynamic.ts` — dynamic import/registry pattern
- `src/tests/legacy.test.ts` — test helper

### 5. New Tests

~12 MCP integration tests covering:
- Basic Rust fixture reachability
- TypeScript fixture entry detection
- Unreachable symbol detection
- Entry points never in unreachable set
- Public API caution
- includeTests=false/true behavior
- compact shape verification
- dead_code_candidates integration
- review_plan integration

## Supported Edge Types for Traversal

- CALLS
- REFERENCES
- IMPORTS / IMPORT
- INCLUDES / INCLUDE
- DEFINES (file → symbol)

## Not Supported

- Runtime control flow
- Type inference
- Macro expansion
- Dynamic dispatch resolution
- Reflection analysis
- Plugin system analysis
- Cross-crate analysis without workspace mode

## Confidence / Caution Strategy

- Entry points: high (exact name match like `main`), medium (file suffix + public symbol), low (high fan-out only)
- Unreachable candidates: cautions for public API, dynamic patterns, framework entries, tests
- All output: `generatedFrom.runtimeVerified = false`, `heuristic = true`

## Verification Plan

1. `cargo fmt --check` + `git diff --check`
2. `cargo test --test mcp_server`
3. `cargo test --test mcp_server --features tree-sitter-typescript`
4. `cargo test --all-features`
5. `scripts/codelattice-mcp.sh --self-test` (≥ 31 tools)
6. `scripts/mcp-dogfood.sh` (all checks pass)
7. Tool index refresh
8. Commit + push gitcode master

## File Change Set

- `crates/cli/src/mcp_server.rs` — new tool schema + handler + enhanced helpers
- `crates/cli/tests/mcp_server.rs` — new tests
- `fixtures/typescript/reachability-map/` — new fixture (9 files)
- `scripts/mcp-dogfood.sh` — new check
- `scripts/codelattice-mcp.sh` — threshold ≥ 31
- `CHANGELOG.md` — Unreleased entry
- `README.md` — new tool description
- `docs/architecture/mcp-v0-contract.md` — new tool section

## Stop Line

If any baseline fails, stop and report. Do not layer changes on broken state.
