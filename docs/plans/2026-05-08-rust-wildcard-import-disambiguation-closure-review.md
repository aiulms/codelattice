# Rust Wildcard Import Source-Aware Disambiguation Closure Review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 2 — Rust CALLS resolution quality
**Commits：** （待 commit）

---

## 总结

本轮在 `resolve_free_function` 的 crate-wide search 之后新增 wildcard import 源模块感知消歧步骤，解决同名函数在多个 same-crate 模块中存在时因 no-edge 策略无法解析的问题。

## 修改内容

### 问题

`calls.rs` 中 `use crate::stdlib_tables::*;` 导入 stdlib_tables 模块，但 `split_last_segment` 函数在 `imports.rs` 和 `stdlib_tables.rs` 各有一个定义。crate-wide search 找到 2 个 match → no-edge（ambiguous），导致 5 个 `split_last_segment` 调用无法解析。

### 根因

`lookup_crate_wide_function` 做全 crate 搜索时，无法区分哪个 match 来自 wildcard-imported 模块。

### 修复

**1. 新增 `build_wildcard_module_map()` 函数**

从已解析 ImportUse 中提取 wildcard/glob import（`original_path` 以 `"::*"` 结尾），构建 caller source_path → 规范化模块路径的映射。

规范化策略：
- 含 `"::"` 的路径（如 `crate::stdlib_tables::*`）→ 直接去掉 `::*` → `crate::stdlib_tables`
- 裸名称（如 `calculations::*`）→ 基于 caller 的 module_path 构建完整路径 → `crate::calculations`

**2. `CalleeIndex` 新增 `wildcard_modules` 字段**

`HashMap<String, HashSet<String>>` — caller source_path → wildcard-imported 模块路径集合。
在 `extract_and_resolve_calls` 中构建并存储到 CalleeIndex。

**3. `resolve_free_function` 新增 step 2.5b — wildcard import 消歧**

在 crate-wide search 返回多个 match 时：
- 检查 caller 是否有 wildcard import 的模块
- 若有，过滤出 module_path 匹配 wildcard-imported 模块的 match
- 若恰好 1 个，解析之（confidence 0.80, reason: call-same-crate-resolved）
- 若 0 个或多个，保持 no-edge（不改语义）

## 影响

| 指标 | Before | After | Delta |
|------|--------|-------|-------|
| Resolution rate | 65.6% (2321/3539) | 65.7% (2338/3557) | +0.1pp |
| call-same-crate-resolved | 18 | 23 | +5 |
| split_last_segment resolved | 1/6 | 6/6 | +5 |

**5 个新解析的 `split_last_segment` 调用全部通过 wildcard-aware 消歧解析到 `stdlib_tables.rs`。**

### 新增 Fixture

`c14-wildcard-disambiguation`（compile-valid，2 calls）：
- `helper_func()` — 同名函数在 calculations.rs 和 utils.rs 中，caller 通过 `use calculations::*;` 消歧
- `process()` — 唯一 match（只在 calculations.rs 中定义）

## 验证

- `cargo fmt --check` ✅ clean
- `git diff --check` ✅ clean
- `cargo test` ✅ 全部通过（no-feature）
- `cargo test --features tree-sitter-cangjie` ✅ 全部通过
- `cangjie_inspect` ✅ 18/18 pass
- `graph_contract` ✅ 24/24 pass
- `project_model_call_expected_compare` ✅ 7/7 pass（含 c14）
- 对 gitnexus-rust-core 自身验证：`split_last_segment` 6/6 resolved ✅

## Stop-lines 合规

- ✅ 未展开 wildcard import（只利用其源模块信息消歧）
- ✅ 未做 type inference / trait solving
- ✅ 未做 macro expansion
- ✅ 未新增依赖
- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未做 destructive git 操作

## 残留限制

1. **消歧仅基于 module_path 精确匹配**：如果 wildcard import 源模块和 callee 的 module_path 不完全一致（如 re-export 导致的路径差异），仍无法消歧
2. **多 wildcard import 指向同一 callee**：若两个 wildcard import 分别从不同模块导入同名函数，当前返回 2 个 preferred match → no-edge（保守策略，正确）
3. **calls.rs 仍有 21 个 unresolved free-function call**：`add` ×8（HashSet::add 误分类）、`cleanup` ×3（runner.rs 同文件多定义）、等

## 下一轮 Opening

**Priority 2 续 — Rust CALLS resolution quality**
- `crate::` 多段路径分类修复（associated-function 误分类为 qualified-path，2 calls）
- 扩张 method call resolution（stdlib type methods 扩展）
- 继续分析 21 个 remaining unresolved free-function calls

**或 Priority 3 续 — 第 4 个 Rust contract fixture**
