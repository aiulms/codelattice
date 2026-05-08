# Preflight: Enum variant 提取 + Type::UpperCaseVariant 分类修复

日期：2026-05-08
状态：Preflight
优先级：Priority 1 — Rust CALLS resolution quality

## 问题描述

两个 `CangjieParseError::ParseFailed(...)` 调用被误分类为 `AssociatedFunction`，无法解析。

根因链：
1. **item.rs** 中 `enum_item` 处理只提取 enum 本身（SymbolKind::Enum），不提取 variant 子符号
   → CalleeIndex 中不存在 `ParseFailed` symbol
2. **calls.rs** `classify_callee` 对 2 段 `scoped_identifier`（如 `CangjieParseError::ParseFailed`）
   只检查第一段是否大写 → 全部分类为 `AssociatedFunction`
   → 即使 variant 在 index 中，也不会被 `resolve_associated_function` 找到（该方法过滤 impl-method 匹配）

Rust 命名约定：
- Associated function：`Type::snake_case_name()` → 第二段小写
- Enum variant：`Enum::PascalCaseName(...)` → 第二段大写

## 修复

### Part 1：提取 enum variant 为 symbol（item.rs）

在 `enum_item` 处理中，递归查找 `enum_variant_list` → `enum_variant` 子节点，
为每个 variant 创建 symbol（SymbolKind: `enum-variant`，parentId: enum symbol）。

### Part 2：修复 classification（calls.rs classify_callee）

对 2 段 `scoped_identifier`（当前第 908-915 行）：
- 原：第一段大写 → AssociatedFunction
- 新：第一段大写 + 第二段大写 → FreeFunction（enum variant）；第一段大写 + 第二段小写 → AssociatedFunction

对 `crate::` 多段路径（当前第 882-895 行）：
- 类似逻辑：第二段最后大写 + 最后一段也大写 → FreeFunction

分类为 FreeFunction 后，`resolve_free_function` 的跨文件搜索会找到 variant symbol。

## Write Set

- `crates/project-model/src/item.rs`：在 `enum_item` 分支中提取 variant
- `crates/project-model/src/calls.rs`：修改 `classify_callee` 的 2 段 + crate:: 多段逻辑

## Forbidden Set

- 不修改 GitNexus-RC / Tool / live repo
- 不新增依赖
- 不修改 symbol 存储/索引数据结构

## Acceptance Criteria

1. `CangjieParseError::ParseFailed(...)` 2 个调用从 unresolved → resolved
2. enum variant 符号出现在 graph 输出中
3. 所有现有测试通过（no-feature + feature-enabled）
4. graph contract 44/44 通过
5. `cargo fmt --check` + `git diff --check` clean

## Stop-line Check

- No type inference / trait solving ✅（AST 直接提取 variant name）
- No macro expansion ✅
- No cfg evaluator ✅
- No external crate resolution ✅（只提取本地 enum variant）
- No destructive git ✅
