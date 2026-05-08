# Preflight: crate:: 多段路径 AssociatedFunction 误分类修复

日期：2026-05-08
状态：Preflight
优先级：Priority 1 — Rust CALLS resolution quality

## 问题描述

`classify_callee`（tree-sitter 版本）和 `classify_text_callee`（文本 fallback 版本）中，
当调用路径以 `crate::` 开头时，立即返回 `QualifiedPath`，未检查后续段是否包含类型名
（即是否为关联函数调用 `crate::module::Type::method()`）。

`resolve_associated_function` 已经具备处理 `crate::module::Type::method()` 的能力
（分支 B：`type_prefix.starts_with("crate::")`），但因为分类错误，这些调用从未到达该函数。

## 影响评估

- 影响范围：gitnexus-rust-core 自有代码中 ~1-2 个调用
- 风险级别：低（仅改变分类逻辑，不改变解析逻辑）
- 不越过 stop-line（不涉及 macro expansion / type inference / trait solving）

## 修复方案

在 `classify_callee` 第 882-883 行 和 `classify_text_callee` 第 1905-1906 行：

当 `first == "crate"` 时，新增检查：
- 如果 segments.len() >= 4 且倒数第二段首字母大写 → `AssociatedFunction`
- 否则保持现有行为 → `QualifiedPath`

检查条件：`segments.len() >= 4` 确保至少有 `crate::module::Type::method` 四个段。

## Write Set

- `crates/project-model/src/calls.rs`：修改 `classify_callee` 和 `classify_text_callee`

## Forbidden Set

- 不修改 `resolve_associated_function`
- 不修改 `resolve_qualified_path`
- 不新增依赖
- 不修改 GitNexus-RC / Tool / live repo

## Acceptance Criteria

1. `crate::module::Type::method()` 被分类为 `AssociatedFunction`
2. `crate::module::function()` 仍被分类为 `QualifiedPath`
3. 所有现有测试通过
4. `cargo fmt --check` + `git diff --check` clean
5. 不引入新 regression

## Stop-line Check

- No type inference / trait solving ✅
- No macro expansion ✅
- No external crate resolution ✅
- No destructive git ✅
