# Phase 2 Slice 11 Execution Card — Cangjie Import Resolution

日期：2026-05-06
状态：已完成

## Scope

Parse Cangjie import statements from tree-sitter AST and resolve them to project packages using existing cjpm.toml/lock metadata. Emit IMPORTS graph edges.

## Required Write Set

| 文件 | 操作 |
|------|------|
| `crates/cangjie/src/extractors/imports.rs` | 新建（~610 行） |
| `crates/cangjie/src/extractors/mod.rs` | 新增 pub mod imports + re-exports |
| `crates/cangjie/src/graph.rs` | 新增 Imports EdgeKind + emit_cangjie_import_edges() + inspect 集成 |
| `crates/cangjie/src/lib.rs` | Re-export 新类型 |
| `crates/cangjie/tests/import_resolution.rs` | 新建（10 integration tests） |
| `fixtures/cangjie/imports-basic/` | 新建 fixture（cjpm.toml + src/main.cj + src/demo/math/add.cj） |

## Forbidden Write Set

- 不改 manifest.rs / project.rs / references.rs / diagnostics/
- 不新增依赖（纯 stdlib + tree-sitter API + serde 已有）
- 不 spawn cjpm tree 子进程
- 不改 GitNexus-RC runtime / Tool / live repo

## API Design

### New Types

```rust
pub enum ImportVisibility { Public, Protected, Internal, Private }
pub struct PackageAlias { pub package_name, pub alias }
pub struct CangjieImport { pub raw_path, pub visibility, pub is_wildcard, pub package_alias, pub file_path }
pub struct ImportCandidate { pub package_name, pub exported_name, pub local_name }
pub struct ResolvedImport { pub target_package_name, pub target_dir, pub resolution }
pub enum ResolutionKind { WorkspaceMember, PathDependency, LockEntry, External }
```

### Public API (feature-gated)

- `extract_cangjie_imports(source, file_path, tree) -> Vec<CangjieImport>` — AST import extraction
- `parse_import_targets(raw) -> Vec<String>` — split raw import path
- `parse_named_import_candidates(raw) -> Vec<ImportCandidate>` — parse named candidates
- `resolve_import_target(candidate, project) -> Option<ResolvedImport>` — resolve to package

### Graph Integration

- `EdgeKind::Imports` — SourceFile → Package or SourceFile → SourceFile
- `emit_cangjie_import_edges(imports_by_file, project) -> Vec<GraphEdge>`

## Test Plan

Unit tests（25）：
- parse_import_targets: single, grouped, wildcard, alias, empty
- parse_named_import_candidates: simple, grouped, alias, wildcard, public_prefix, empty
- is_external_package: std, core, normal
- is_valid_identifier: valid, invalid
- split_top_level_comma: simple, nested braces, single
- strip_alias: simple, none
- package_name_from_target: simple, wildcard, grouped

Integration tests（10）：
- fixture_main_parses_cleanly, fixture_add_parses_cleanly
- imports_are_extracted, single_import_has_correct_path
- wildcard_import_detected, public_import_detected
- full_project_graph_contains_import_edges, project_graph_contains_all_node_types
- import_edges_have_valid_structure, output_is_deterministic

## Acceptance Criteria

- [x] cargo fmt --check clean
- [x] cargo check pass（without feature）
- [x] cargo test pass（179/179 without feature）
- [x] cargo test --features tree-sitter-cangjie pass（209/209）
- [x] 新增 fixture 被正确解析（无 ERROR nodes）
- [x] 35 new tests（25 unit + 10 integration）
- [x] IMPORTS edges 出现在 graph output JSON
- [x] 零新增依赖

## Verification Results

| Check | Result |
|-------|--------|
| cargo fmt --check | Clean |
| cargo test | 179/179 pass |
| cargo test --features tree-sitter-cangjie | 209/209 pass |
| 新增依赖 | 零 |

## Known Limitations

- Same-project resolution only（workspace member + path dep + static lock entry）
- 不解析 git-based dependency（需要 cjpm tree 或 network clone）
- cjpm tree subprocess 未实现（deferred to Slice 11b）
- 不扩展 reference extraction 到 cross-file（deferred to Slice 12）

## Next

Slice 11b — cjpm tree subprocess + external dependency resolution, 或 Slice 12 — cross-file reference extraction.
