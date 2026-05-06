# Cangjie Phase 2 Slice 12 — Cross-File Reference Extraction Closure Review

**Date:** 2026-05-06
**Status:** Complete
**Author:** aiulms

## Outcome

Cross-file reference extraction MVP implemented and verified.

## Changes Summary

### Core implementation

**references.rs** — Added cross-file resolution infrastructure:
- `CangjieReference::target_file: Option<String>` — new field for cross-file target file
- `CrossFileSymbolIndex` — project-wide symbol index, keyed by `(file_path, symbol_name)`
- `ImportBinding` / `ImportBindingTable` — maps `(source_file, local_name)` → resolved cross-file target
- `push_reference()` — now tries same-file first (SameFileIndex), then cross-file fallback (ImportBindingTable)
- Cross-file resolved references get confidence 0.85 and reason suffix "(cross-file via import)"

**graph.rs** — Pipeline integration:
- `inspect_cangjie_project()` — restructured to extract imports first, build ImportBindingTable, then extract references with cross-file support
- `emit_cangjie_reference_edges()` — uses `target_file` for cross-file symbol lookup when available

**mod.rs** — Updated re-export of `extract_cangjie_references` to match new signature.

### Tests

**cross_file_reference.rs** (new integration tests, feature-gated):
1. `inspect_project_produces_graph_with_cross_file_references` — full pipeline test
2. `cross_file_reference_resolves_point_type_annotation` — targeted: with/without bindings comparison, cross-file Point reference verification
3. `reference_targets_exist_in_graph` — endpoint integrity: all reference targets exist in graph

**reference_extraction.rs** — Updated call to pass `None` for import_bindings parameter.

### Fixture

**fixtures/cangjie/reference-cross-file-basic/** — Minimal cross-file reference fixture:
- `cjpm.toml` — package reference-cross-file-basic
- `src/main.cj` — imports `Point` from mathpkg.ops, uses it as type annotation
- `src/mathpkg/ops.cj` — defines `Point` class and `add` function

## Architecture Verification

- All 95 tests pass without feature `tree-sitter-cangjie`
- All 108 tests pass with feature (105 existing + 3 new cross-file)
- `cargo fmt --check` clean
- `cargo check` clean (only pre-existing dead_code warnings)
- Zero new dependencies
- Feature gate maintained — all cross-file logic behind `#[cfg(feature = "tree-sitter-cangjie")]`

## Supported Reference Forms

| Form | Status |
|------|--------|
| Same-file reference (type annotation, field read, write) | Unchanged |
| Cross-file reference via explicit named import | NEW — confidence 0.85 |
| Cross-file reference via grouped import `{a, b}` | NEW — each individually resolved |

## Unsupported (by design, MVP scope)

- Wildcard import expansion (`import pkg.*`)
- Alias renamed import (`import pkg.{Foo as Bar}`)
- Method dispatch on imported types
- Type inference / trait solving
- Macro expansion
- External version/git dependency symbols
- Function call references (AST walk only extracts type annotations, field reads, writes)

## Known Limitations

1. **Function call references not extracted**: The AST walk only extracts type annotations (USES), field reads (ACCESSES), and writes (MODIFIES). Function calls like `add(1, 2)` are not extracted as references. This is a Slice 10 limitation, not introduced by Slice 12.
2. **Endpoint integrity — source_id mismatch**: Reference `source_id` format (`Function:<path>:<name>`) doesn't match graph symbol node ID format (`sym:<rel-path>:<name>#<line_range>`). This is a pre-existing design issue, not introduced by Slice 12. Target endpoint integrity is verified and passes.
3. **1 pre-existing dead_code warning**: `package_name_from_target()` in imports.rs (Slice 11 era, not consumed by Slice 12).

## Files Modified

| File | Lines changed |
|------|---------------|
| `crates/cangjie/src/extractors/references.rs` | ~200 added (index structures + cross-file fallback) |
| `crates/cangjie/src/extractors/mod.rs` | ~2 (re-export signature) |
| `crates/cangjie/src/graph.rs` | ~30 (restructured pipeline) |
| `crates/cangjie/tests/reference_extraction.rs` | 1 (add None parameter) |
| `crates/cangjie/tests/cross_file_reference.rs` | 265 (new) |
| `fixtures/cangjie/reference-cross-file-basic/` | 3 files (new fixture) |
| `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-preflight.md` | New |
| `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-execution-card.md` | New |
| `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-closure-review.md` | This file |

## Forbidden Boundaries — Verified Untouched

- GitNexus-RC runtime (TS code): NOT MODIFIED
- GitNexus-RC-Tool checkout: NOT MODIFIED
- Cangjie live repo: NOT MODIFIED
- Cangjie index checkout: READ-ONLY, NOT MODIFIED
- MCP server / HTTP API / UI: NOT MODIFIED
- Cangjie LSP client: NOT MODIFIED
- Diagnostics runner: NOT MODIFIED
- Schema migration: NOT PERFORMED

## Risks

No new HIGH or CRITICAL risks. All Slice 12 changes are within the existing `tree-sitter-cangjie` feature gate and follow established patterns.

## Recommendation

Slice 12 is complete. Next opening:
- **Slice 13**: Expand reference extraction to include function call references (USES edges for imported function calls)
- OR: Address pre-existing source_id format mismatch (make reference source_ids use symbol node ID format)
- OR: Begin wildcard import expansion (requires preflight first)
