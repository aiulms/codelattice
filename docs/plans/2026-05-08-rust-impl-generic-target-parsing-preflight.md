# Preflight: impl 块泛型目标类型解析修复

日期：2026-05-08
状态：Preflight
优先级：Priority 1 — Rust CALLS resolution quality

## 问题描述

`parse_impl_header()` 在解析 `impl<'a> SameFileIndex<'a>` 时，tree-sitter 将
`SameFileIndex<'a>` 解析为 `generic_type` 节点（含 `type_identifier` 子节点），
但该函数直接跳过 `generic_type` 节点，导致 `impl_target` 未找到，fallback 为 `"Unknown"`。

后果：
- impl block symbol 名为 `_impl_Unknown` 而非 `_impl_SameFileIndex`
- `SameFileIndex::build()` 关联函数无法通过 impl_target 过滤找到
- 5 个 `SameFileIndex::build()` 调用全部 unresolved

## 根因

`parse_impl_header` 的 match 语句跳过 `generic_type` 和 `scoped_type_identifier` 节点，
不递归查找其子节点。对于 `impl<'a> SameFileIndex<'a>`：
- `SameFileIndex<'a>` → tree-sitter 解析为 `generic_type`
- 该 `generic_type` 含 `type_identifier` 子节点 "SameFileIndex"
- 因 `generic_type` 被跳过，无法提取类型名

## 修复

在 `parse_impl_header` 中，对于 `generic_type` 和 `scoped_type_identifier` 节点，
递归查找其直接子节点中的 `type_identifier`，并照常处理。

## Write Set

- `crates/project-model/src/item.rs`：修改 `parse_impl_header`

## Forbidden Set

- 不修改 calls.rs / graph.rs
- 不新增依赖
- 不修改 GitNexus-RC / Tool / live repo

## Acceptance Criteria

1. `impl<'a> SameFileIndex<'a>` 的 impl block symbol 名为 `_impl_SameFileIndex`
2. `SameFileIndex::build()` 5 个调用全部解析
3. 所有现有测试通过
4. `cargo fmt --check` + `git diff --check` clean

## Stop-line Check

- No type inference / trait solving ✅
- No macro expansion ✅
- No destructive git ✅
