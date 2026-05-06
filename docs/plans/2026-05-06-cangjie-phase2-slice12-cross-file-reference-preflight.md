# Cangjie Phase 2 Slice 12 — Cross-File Reference Extraction Preflight

**Date:** 2026-05-06
**Status:** Preflight
**Author:** aiulms
**Prerequisite slices:** Slice 1-11b (all complete)

## Phase 0 Audit Summary

### Current state

Slice 10 (same-file references) + Slice 11/11b (import resolution) are complete.
references.rs can extract same-file USES/ACCESSES/MODIFIES edges at confidence 0.60–0.85.
imports.rs can resolve import targets to package directories via 4-level fallback.
What's missing: the bridge between imported names and cross-file symbol targets.

### references.rs current architecture

- `SameFileIndex`: Builds from `Vec<CangjieSymbol>` per file, keyed by `name`.
  - `resolve()` returns `Some` only on unique match; ambiguous → `None` (no edge emitted).
- `push_reference()`: Calls `self.index.resolve(name, &target_kinds)` — **same-file only**.
- `BUILTIN_TYPES` (25 types): Filtered out; `type_name_kind()` maps parent_kind → declaration kind.
- `FuncContext`: Tracks enclosing function for method scoping (not yet wired to cross-file).
- `build_source_id()`: Generates Constructor/Method/Function node IDs.
- All main logic behind `#[cfg(feature = "tree-sitter-cangjie")]`.
- **Bottleneck**: If `SameFileIndex::resolve()` returns `None`, no edge is emitted — even when
  the target symbol exists in another file that is imported by the current file.
- 8 dead_code warnings: `BUILTIN_TYPES`, `is_builtin_type`, `TYPE_DECLARATION_KINDS`,
  `type_name_kind`, `FuncContext`, `SameFileIndex` (struct + build + resolve) — all are
  used inside `#[cfg(feature = "tree-sitter-cangjie")]` blocks but defined outside them.

### imports.rs current architecture

- `parse_import_targets()`: Parses import statements → Vec of `(raw_path, visibility, is_wildcard, package_alias)`.
- `parse_named_import_candidates()`: From raw path → Vec<ImportCandidate>.
- `resolve_import_target()`: Maps candidate to `ResolvedImport { target_package_name, target_dir, resolution }`.
- `candidate_package_dirs()`: 4-level fallback — workspace member → path dep → lock entry → tree dep.
- `extract_cangjie_imports()`: AST-based import extraction (feature-gated).
- `package_name_from_target()`: Extracts root package name from import path (currently test-only).
- `is_tree_dependency_match()`: Checks if a directory matches a cjpm tree dependency.
- Integration: `inspect_cangjie_project()` calls extract imports → resolve → emit import edges.

### project.rs capabilities

- `CangjieProject` has `source_files: Vec<PathBuf>` — all .cj files in workspace.
- `packages: Vec<CangjiePackageInfo>` — each with name, module_dir, src_dir.
- Can serve as basis for project-wide symbol index.

### graph.rs reference-edge emission

- `emit_cangjie_reference_edges()`: Builds `HashMap<(String, String), String>` from
  `symbols_by_file` — key is `(file_path, symbol_name)` → node_id.
- Lookup: `(r.file_path.clone(), r.target_name.clone())` — **same-file only**.
- Cross-file would need to look up targets from other files' symbol maps.

### cjpm_tree.rs

- `run_cjpm_tree()`, `find_package_dir_by_name()`, `resolve_tree_dependency_dir()`.
- Used by imports.rs for tree dependency resolution.
- Could also help cross-file reference find external package source dirs.

### cargo status

```
cargo fmt --check: clean
cargo check: 8 warnings (all references.rs dead_code, Slice 12 will consume)
cargo test: 95/95 pass (without feature)
cargo test --features tree-sitter-cangjie: 105/105 pass
HEAD: 25c7333
```

---

## 1. MVP Scope

Cross-file reference extraction for **explicit imports with exact name match**.

When file A imports a specific name from file B (`import pkg.helper.{add}`), and
a reference in file A uses `add`, produce a USES edge from file A's reference site
to the `add` symbol defined in file B.

**Concrete MVP deliverable:**
1. Build a project-wide `CrossFileSymbolIndex` from all source files' extracted symbols.
2. Build an `ImportBindingTable` from imports.rs resolution results.
3. Modify `push_reference()` in references.rs: when `SameFileIndex` misses, fall back
   to import bindings → cross-file symbol lookup.
4. Produce reference edges at confidence 0.85 for cross-file resolved targets.
5. One new fixture: `fixtures/cangjie/reference-cross-file-basic/`.

## 2. Supported Reference Forms (MVP)

| Form | Example | Support |
|------|---------|---------|
| Simple import — type/function name | `import demo.helper` → `helper.foo()` | YES |
| Explicit grouped import — exact name | `import demo.helper.{add, sub}` → `add()` | YES |
| Explicit single import | `import demo.helper.{add}` | YES |
| Same-file reference (existing) | `let x: Foo = ...` where Foo in same file | YES (unchanged) |

## 3. Unsupported Forms (MVP)

| Form | Reason |
|------|--------|
| Wildcard import expansion (`import pkg.*`) | Would require scanning all exports of target package; complex |
| Alias renamed import (`import pkg.{Foo as Bar}`) | Name mapping requires import statement AST re-parsing |
| Method dispatch on imported type | Requires type inference |
| Type inference / trait solving | Stop-line |
| Macro expansion | Stop-line |
| External version/git dependency symbols | No source-code access |
| Full package semantic / re-export chains | Beyond MVP scope |

## 4. Required Index Structures

### CrossFileSymbolIndex

```
Key: (file_path: String, symbol_name: String, symbol_kind: CangjieSymbolKind)
Value: CangjieSymbol
```

Built once at the start of reference extraction from all `project.source_files`.
Provides O(1) lookup for any (file, name) pair.

### ImportBindingTable

```
Key: (source_file: String, local_name: String)
Value: Vec<ResolvedBinding> {
    target_file: PathBuf,       // specific .cj file containing the symbol
    target_package_dir: PathBuf, // src-dir of the target package
    resolution_kind: ResolutionKind,
}
```

Built from `extract_cangjie_imports()` + `parse_named_import_candidates()` +
`resolve_import_target()`.

For each import in each file:
1. Parse import candidates (named imports only — skip wildcard).
2. Resolve to package directory via `candidate_package_dirs()`.
3. Find candidate source files in the target directory.
4. Cross-reference with symbol names extracted from those files.
5. Record binding: `(importing_file, imported_name) → [(target_file, ...)]`.

If multiple candidate target files exist, resolve to the one that actually contains
the named symbol, or mark as ambiguous (no edge emitted).

## 5. Output Edges

- Continue using existing `USES`, `ACCESSES`, `MODIFIES` EdgeKind.
- Same-file exact: confidence 0.90 (unchanged).
- Explicit import exact: confidence 0.85 (new).
- Ambiguous (multiple candidates): no edge emitted.
- Unresolved (no candidate): no edge emitted.
- `reason` field: "same-file" vs "cross-file import resolve" with package/import path.
- `reason` field format: `"cross-file import resolve: pkg.helper via workspace member"`
- All edges must have valid source and target node IDs (endpoint integrity).

## 6. Required Write Set

| File | Change |
|------|--------|
| `crates/cangjie/src/extractors/references.rs` | Add CrossFileSymbolIndex, modify push_reference() to try cross-file fallback |
| `crates/cangjie/src/extractors/imports.rs` | Expose helper to build ImportBindingTable from resolved imports (may need to make `package_name_from_target` pub or add new public function) |
| `crates/cangjie/src/graph.rs` | Expand symbol lookup in `emit_cangjie_reference_edges()` to support cross-file targets |
| `crates/cangjie/src/project.rs` | Possibly add helper to build project-wide symbol index |
| New fixture: `fixtures/cangjie/reference-cross-file-basic/` | cjpm.toml, src/main.cj, src/helper.cj + expected output |
| Tests in `references.rs` (or new integration test file) | Cross-file resolve, missing import, ambiguous import, endpoint integrity |
| `docs/plans/README.md` (both repos) | Update after closure |

## 7. Forbidden Write Set

| Boundary | Status |
|----------|--------|
| GitNexus-RC runtime (TS code) | FORBIDDEN |
| GitNexus-RC-Tool checkout | FORBIDDEN |
| Cangjie live repo (`/Users/jiangxuanyang/Desktop/cangjie`) | FORBIDDEN |
| Cangjie index checkout (`cangjie-GitNexus-Index`) | READ-ONLY |
| MCP server / HTTP API / UI | STOP-LINE |
| Cangjie LSP client | STOP-LINE |
| Diagnostics runner modifications | FORBIDDEN |
| Schema migration | FORBIDDEN |
| `target/`, `.arts/`, `.claude/`, `.codebuddy/`, `.qoder/`, `skills/` | NEVER COMMIT |

## 8. Acceptance Criteria

1. `cargo fmt --check` clean.
2. `cargo check` — 0 new errors; existing 8 dead_code warnings may decrease as Slice 12
   consumes SameFileIndex, FuncContext, and related helpers.
3. `cargo test` — all existing tests pass (95/95 without feature).
4. `cargo test --features tree-sitter-cangjie` — all existing + new cross-file tests pass.
5. New cross-file fixture passes:
   - `main.cj` imports `helper.cj` function → reference resolves to helper.cj symbol.
   - Missing import → no fake edge.
   - Ambiguous import (same name in two imported files) → no edge.
6. Endpoint integrity: every edge source/target exists in graph node set.
7. Confidence/reason fields documented and populated correctly.
8. Zero new dependencies.
9. Feature gate maintained: cross-file resolution only active with `tree-sitter-cangjie`.

## 9. Risk Assessment

| Risk | Level | Mitigation |
|------|-------|------------|
| Symbol name collision across files | MEDIUM | Only emit edge on unique match; ambiguous → None |
| Import resolution failure (SDK absent) | LOW | Graceful degrade — same-file references still work |
| Performance regression (project-wide index) | LOW | HashMap lookup O(1); project size < 1k files |
| Breaking existing same-file behavior | LOW | SameFileIndex tried first; cross-file is fallback only |
| Endpoint integrity violation | MEDIUM | Validate every edge target exists in graph node set before emitting |

**No HIGH or CRITICAL risks identified.** Slice 12 is a bounded, well-understood
extension of existing infrastructure.

## 10. Decision: Proceed to Execution Card

**Recommendation: PROCEED.**

Slice 12 is a natural extension of Slice 10 (same-file references) and Slice 11/11b
(import resolution). The architecture gap is well-understood (SameFileIndex → cross-file
fallback via ImportBindingTable + CrossFileSymbolIndex). The MVP scope is narrow
(explicit imports, exact names), the forbidden set is clear, and the risk level is low.

The 8 dead_code warnings in references.rs will naturally be consumed by Slice 12
implementation, reducing technical debt.

No preflight blockers identified.
