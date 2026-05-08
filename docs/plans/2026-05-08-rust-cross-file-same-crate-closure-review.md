# Rust 跨文件 Same-Crate Call Resolution Closure Review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 2 — Rust CALLS resolution quality
**Commits：** `55bc86a` `669ddc6`

---

## 总结

本轮在 `resolve_free_function` 和 `resolve_type_module` 中新增跨文件 same-crate 搜索，解决因 wildcard import 不展开导致的跨文件调用无法解析的问题。

## 修改内容

### Slice 2e-1: 跨文件 same-crate free function 解析

**问题：** `calls.rs` 通过 `use crate::stdlib_tables::*;` 导入 stdlib_tables 模块，但 glob import 不展开，导致调用 `lookup_stdlib_trait_method()` 等函数时 same-module 和 import-binding 查找均失败。

**根因：** `resolve_free_function` 的查找链为 same-module → import-binding → same-file → unresolved，缺少跨文件 same-crate 搜索步骤。

**修复：**
- 新增 `CallSameCrateResolved` reason（confidence 0.80）
- `CalleeIndex` 新增 `source_to_package` 映射和 `lookup_crate_wide_function()` 方法
- `resolve_free_function` 在 import binding 之后新增 step 2.5：crate-wide unique function search
- 仅当 crate 内唯一匹配时解析（no-edge 策略：多个 match 时不解析）

### Slice 2e-2: 跨文件 same-crate type 搜索（associated function 辅助）

**问题：** `resolve_type_module` 在 same-module 和 import-binding 都失败后直接返回 None，无法找到跨文件定义的 type。

**修复：**
- `CalleeIndex` 新增 `lookup_crate_wide_type()` 方法
- `resolve_type_module` 新增 step 3：crate-wide type search
- 需要传入 `caller_source_path` 以确定所属 crate

### Slice 2e-3: crate:: 路径 CalleeIndex fallback

**问题：** `resolve_qualified_path` 在 `resolve_module_chain` 失败时直接返回 unresolved，不尝试 CalleeIndex 查找。

**修复：** 在模块链解析失败分支中，添加直接 CalleeIndex 查找 fallback。

### 新增 Fixture

`c13-cross-file-same-crate`（compile-valid，2 calls）：
- `compute_value()` — 跨文件 free function，wildcard import 引入
- `Calculator::new()` — 跨文件 associated function，wildcard import 引入
- 两个调用均通过新增的 crate-wide 搜索解析

## 影响

| 指标 | Before | After | Delta |
|------|--------|-------|-------|
| Resolution rate | 65.0% (2283/3514) | 65.6% (2321/3539) | +0.6pp |
| Cross-file same-crate resolved | 0 | 18 | +18 |
| Unresolved free-function | 38 | 21 | -17 |

**18 个新解析 call 全部来自 project-model crate（calls.rs → stdlib_tables.rs）：**
- `lookup_stdlib_trait_method` ×4
- `scan_variable_type_annotation` ×4
- `lookup_receiver_type_method` ×4
- `strip_generics` ×4
- `lookup_prelude_type_path` ×1

**仍 unresolved 的 21 个 free-function call：**
- `split_last_segment` ×5：两个文件（imports.rs 和 stdlib_tables.rs）各有同名 function，crate-wide 匹配到多个，no-edge 策略正确
- `cleanup` ×3：cangjie 同文件内函数（runner.rs），用相同名称多次定义
- `add` ×8：HashSet::add 被误分类为 free-function
- `tree_sitter_cangjie` ×2：feature-gated 函数，符号在不同 feature 下不可见
- `resolve_import_target` ×2：ambiguous import resolution
- `is_tree_sitter_available` ×1：ambiguous

## 残留限制

1. **Associated function derive-generated 方法无法解析**（如 `#[derive(Default)]` 的 `default()`）：tree-sitter 不提取 compiler-generated 符号
2. **重导出路径**无法在 CalleeIndex 中匹配（如 `pub use runner::fn;` 使 `fn` 在父模块可用，但 CalleeIndex 只有原始定义模块）
3. **`split_last_segment` 歧义**：两个文件各有一个同名 function，wildcard import 源头信息丢失

## 验证

- `cargo fmt --check` ✅ clean
- `git diff --check` ✅ clean
- `cargo test` ✅ 全部通过
- `cargo test --features tree-sitter-cangjie` ✅ 全部通过
- `cangjie_inspect` ✅ 18/18 pass
- `graph_contract` ✅ 24/24 pass
- `multi_project_smoke` ✅ 4/4 fixture pass
- `project_model_call_expected_compare` ✅ 21/21 fixtures pass（含新增 c13）

## Stop-lines 合规

- ✅ 未做 type inference / trait solving
- ✅ 未做 macro expansion
- ✅ 未做 full cfg evaluator
- ✅ 未新增依赖
- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 不做 destructive git 操作

## 下一轮 Opening

**Priority 2 续 — Rust CALLS resolution quality：**
- `split_last_segment` 歧义修复（wildcard import 源模块感知优先）
- `crate::` 多段路径的分类修复（associated-function 误分类为 qualified-path）
- 扩张 graph contract test 覆盖度
- 继续扩大 method call resolution（stdlib type methods 扩展）

**或 Priority 4 — Cangjie maintenance：**
- Quality gate 周期性回归验证
- QUALITY.md 维护
