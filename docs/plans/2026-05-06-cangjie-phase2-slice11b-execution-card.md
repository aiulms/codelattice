# Phase 2 Slice 11b Execution Card — Cangjie cjpm tree subprocess + external dependency resolution

日期：2026-05-06
状态：进行中

## Scope

Port TS cjpm tree subprocess + parser + directory resolver to Rust-core, integrate into import resolution as fourth-level fallback (after workspace member → path dep → lock entry).

## Required Write Set

| 文件 | 操作 |
|------|------|
| `crates/cangjie/src/subprocess/cjpm_tree.rs` | 新建（~200 行） |
| `crates/cangjie/src/subprocess/mod.rs` | 新建（~15 行） |
| `crates/cangjie/src/extractors/imports.rs` | 修改（~80 行）: candidate_package_dirs + ResolutionKind + TreeDependency |
| `crates/cangjie/src/lib.rs` | 新增 pub mod subprocess |
| `crates/cangjie/tests/cjpm_tree.rs` | 新建（~8 integration tests） |
| `fixtures/cangjie/imports-basic/` | 扩展（如需要额外依赖声明） |

## Forbidden Write Set

- 不改 graph.rs / project.rs / manifest.rs / references.rs / diagnostics/
- 不新增依赖（纯 stdlib + serde 已有）
- 不修改 `inspect_cangjie_project()` 签名
- 不改 GitNexus-RC runtime / Tool / live repo

## Implementation Steps

1. Create `crates/cangjie/src/subprocess/mod.rs` — pub mod cjpm_tree
2. Create `crates/cangjie/src/subprocess/cjpm_tree.rs`:
   - `parse_cjpm_tree_output()` — pure text parser
   - `run_cjpm_tree()` — subprocess runner (reuse diagnostics/runner.rs patterns)
   - `is_cjpm_available()` — SDK tool check
   - `find_package_dir_by_name()` — recursive cjpm.toml [package].name search
   - `resolve_tree_dependency_dir()` — workspace subtree search with cache
3. Modify `crates/cangjie/src/extractors/imports.rs`:
   - Add `TreeDependency` to `ResolutionKind`
   - Extend `candidate_package_dirs()` with tree dependency fallback
4. Update `crates/cangjie/src/lib.rs` — add `pub mod subprocess`
5. Create tests

## Verification

- cargo fmt --check
- cargo test (all existing pass)
- cargo test --features tree-sitter-cangjie (all new + existing pass)
- 零新增依赖
