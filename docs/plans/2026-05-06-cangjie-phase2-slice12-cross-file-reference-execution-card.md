# Cangjie Phase 2 Slice 12 — Cross-File Reference Extraction Execution Card

**Date:** 2026-05-06
**Status:** Execution Card
**Author:** aiulms
**Preflight:** `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-preflight.md`

## Implementation Plan

### Step 1: Add `target_file` field to `CangjieReference`

**File:** `crates/cangjie/src/extractors/references.rs`

Add `target_file: Option<PathBuf>` to `CangjieReference`:
- `Some(path)` for same-file and cross-file resolved targets.
- `None` for unresolved targets (skip edge emission).

### Step 2: Build `CrossFileSymbolIndex`

**File:** `crates/cangjie/src/extractors/references.rs`

New struct (behind feature gate or at module level):

```rust
struct CrossFileSymbolIndex {
    // key: (file_path, symbol_name) → symbol
    by_file_and_name: HashMap<(String, String), CangjieSymbol>,
}

impl CrossFileSymbolIndex {
    fn build(symbols_by_file: &HashMap<PathBuf, Vec<CangjieSymbol>>) -> Self;
    fn resolve(&self, file: &Path, name: &str, kinds: &[CangjieSymbolKind]) -> Option<&CangjieSymbol>;
    fn find_symbol_in_dir(&self, dir: &Path, name: &str, kinds: &[CangjieSymbolKind]) -> Vec<&Path>; // candidate files
}
```

### Step 3: Build `ImportBindingTable`

**File:** `crates/cangjie/src/extractors/references.rs` (new module or inline)

```rust
struct ImportBindingTable {
    // key: (source_file, local_name) → Vec<(target_file, symbol)>
    bindings: HashMap<(String, String), Vec<(PathBuf, CangjieSymbol)>>,
}

impl ImportBindingTable {
    fn build(
        source_files: &[PathBuf],
        symbols_by_file: &HashMap<PathBuf, Vec<CangjieSymbol>>,
        cross_file_index: &CrossFileSymbolIndex,
    ) -> Self;
    fn resolve(&self, source_file: &str, name: &str, kinds: &[CangjieSymbolKind]) -> Option<&(PathBuf, CangjieSymbol)>;
    // Returns Some only on unique match
}
```

**Build algorithm:**
1. For each source file, extract imports via `extract_cangjie_imports()`.
2. For each import, call `parse_named_import_candidates()` → candidates.
3. For each candidate, call `resolve_import_target()` → `ResolvedImport`.
4. If `resolved.target_dir` is Some:
   - Use `CrossFileSymbolIndex::find_symbol_in_dir()` to find candidate files.
   - If exactly one file defines the symbol → record binding.
   - If zero or multiple → skip (no binding).
5. Package alias: treat `import pkg.helper` → name binding `helper` as a "module alias".
   References to `helper.foo` would be resolved by looking for `foo` in files under the resolved package dir.

### Step 4: Modify `push_reference()` — cross-file fallback

**File:** `crates/cangjie/src/extractors/references.rs`

Current logic:
```
if let Some(target) = self.index.resolve(name, &target_kinds) {
    // emit edge (same-file, 0.90)
}
```

New logic:
```
if let Some(target) = self.index.resolve(name, &target_kinds) {
    // emit edge (same-file, confidence 0.90, target_file = current file)
} else if let Some((target_file, target)) = self.import_bindings.resolve(...) {
    // emit edge (cross-file, confidence 0.85, target_file = imported file)
}
// else: skip (no fake edge)
```

### Step 5: Modify `extract_cangjie_references()` signature

Add parameters:
- `cross_file_index: &CrossFileSymbolIndex`
- `import_bindings: &ImportBindingTable`

These are built once at the `inspect_cangjie_project()` level and passed through.

### Step 6: Modify `emit_cangjie_reference_edges()` — cross-file lookup

**File:** `crates/cangjie/src/graph.rs`

Current lookup: `(r.file_path.clone(), r.target_name.clone())`
New lookup: Use `r.target_file` if set, fall back to `r.file_path`:

```rust
let lookup_file = r.target_file.as_deref().unwrap_or(&r.file_path);
let target_node_id = symbol_node_ids.get(&(lookup_file.to_string_lossy().to_string(), r.target_name.clone()));
```

### Step 7: New fixture

**Directory:** `crates/cangjie/test/fixtures/cangjie/reference-cross-file-basic/`

```
cjpm.toml           — workspace with packages: main, helper
src/main.cj         — imports helper.add → calls add(1, 2)
src/helper.cj       — defines func add(a: Int64, b: Int64): Int64
```

**Expected behavior:**
- `add` reference in main.cj resolves to helper.cj `add` definition.
- Confidence: 0.85.
- Reason: "cross-file import resolve: helper via workspace member".

### Step 8: Integration in `inspect_cangjie_project()`

**File:** `crates/cangjie/src/graph.rs`

After symbol extraction, before reference extraction:
1. Build `symbols_by_file: HashMap<PathBuf, Vec<CangjieSymbol>>`.
2. Build `CrossFileSymbolIndex` from symbols_by_file.
3. Build `ImportBindingTable` from imports + cross_file_index.
4. Pass both to `extract_cangjie_references()`.

### Step 9: Tests

**Unit tests** (in references.rs, feature-gated):
1. `cross_file_index_build_and_resolve` — basic lookup works.
2. `cross_file_index_ambiguous` — multiple matches → None.
3. `import_binding_table_exact_match` — explicit import → binding found.
4. `import_binding_table_missing_symbol` — no symbol in target → no binding.
5. `import_binding_table_ambiguous` — symbol in multiple target files → no binding.

**Integration tests** (new file or in existing test module):
6. `cross_file_reference_resolves` — end-to-end: main.cj reference → helper.cj symbol.
7. `cross_file_reference_missing_import` — unresolved import → no edge.
8. `cross_file_endpoint_integrity` — all edge targets exist in graph.

### Step 10: Verification

```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo check
cargo test
cargo test --features tree-sitter-cangjie
```

## Write List

| File | Action |
|------|--------|
| `crates/cangjie/src/extractors/references.rs` | Add CrossFileSymbolIndex, ImportBindingTable, modify push_reference(), add target_file to CangjieReference |
| `crates/cangjie/src/graph.rs` | Expand symbol lookup for cross-file targets, build indices in inspect_cangjie_project() |
| `crates/cangjie/test/fixtures/cangjie/reference-cross-file-basic/cjpm.toml` | New |
| `crates/cangjie/test/fixtures/cangjie/reference-cross-file-basic/src/main.cj` | New |
| `crates/cangjie/test/fixtures/cangjie/reference-cross-file-basic/src/helper.cj` | New |
| `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-preflight.md` | Already written |
| `docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-execution-card.md` | This file |

## Forbidden Write List

Same as preflight — see Section 7 of preflight document.

## Estimated Lines of Change

- references.rs: ~150 lines added (CrossFileSymbolIndex + ImportBindingTable + modified push_reference)
- graph.rs: ~30 lines modified (expanded lookup + index building in inspect_cangjie_project)
- Fixtures: ~30 lines total (cjpm.toml + 2x .cj files)
- Tests: ~100 lines

**Total: ~310 lines. Zero new dependencies.**
