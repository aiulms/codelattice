# Rust Wildcard Import Source-Aware Disambiguation Preflight

**日期：** 2026-05-08
**状态：** 实现中
**类型：** Priority 2 — Rust CALLS resolution quality

---

## 问题

`calls.rs` 中使用 `use crate::stdlib_tables::*;` 引入 stdlib_tables 模块，但 wildcard import 不展开（stop-line: no macro expansion / no glob expansion），导致调用 `split_last_segment()` 等函数时 crate-wide search 在 `imports.rs` 和 `stdlib_tables.rs` 两个文件中各找到同名函数，触发 no-edge 策略（多个 match → unresolved）。

当前 5 个未解析的 `split_last_segment` call：
- `calls.rs` → `split_last_segment`：imports.rs（line 477）和 stdlib_tables.rs（line 134）各有一个同名函数

## 根因

`lookup_crate_wide_function` 做全 crate 搜索，按 name + symbol_kind + same package + different file 过滤，但不感知 wildcard import 的源模块信息。当两个文件各有一个同名函数时，返回 2 个 match → no-edge。

## 方案

### 思路

当 crate-wide search 返回多个 match 时，若 caller 有 wildcard import，利用 wildcard import 的 `original_path` 来辨别"哪个 match 来自 wildcard 导入的模块"：
- `use crate::stdlib_tables::*;` 的 `original_path = "crate::stdlib_tables"`
- 在 `stdlib_tables.rs` 中定义的函数其 `module_path = "crate::stdlib_tables"`
- 在 `imports.rs` 中定义的函数其 `module_path = "crate"` 或 `"crate::imports"`

因此可以通过 `module_path` 匹配来区分。

### 数据流

1. **构建 `caller_wildcard_modules`**：从已解析 ImportUse 中提取 wildcard import（`path_kind == "use_wildcard"`），构建 `HashMap<String, HashSet<String>>` — caller source_path → wildcard-imported module original_path 集合
2. **传递 wildcard_modules 到 resolve**：在 `resolve_free_function` 和 `lookup_crate_wide_function` 中增加 wildcard_modules 参数
3. **多 match 时过滤**：当 crate-wide search 返回多个 match，尝试按 wildcard_modules 过滤 → 若恰好 1 个 match，解析；否则保持 no-edge

### Confidence

保持 0.80（CallSameCrateResolved），不提高。因为 wildcard import 不展开是已知限制，源模块匹配是 heuristic。

### 写集

| 文件 | 修改内容 |
|------|---------|
| `crates/project-model/src/calls.rs` | 新增 wildcard_modules 构建 + lookup_crate_wide_function 参数扩展 |
| `fixtures/call-resolution/c14-wildcard-disambiguation/` | 新增 fixture（compile-valid） |
| `crates/cli/tests/project_model_call_expected_compare.rs` | 添加 c14 fixture |

### 禁止集

- 不展开 wildcard import（stop-line）
- 不做 type inference / trait solving
- 不修改 GitNexus-RC / Tool / live repo
- 不新增依赖

## 验证

- `cargo fmt --check` + `git diff --check`
- `cargo test`（no-feature）
- `cargo test --features tree-sitter-cangjie`
- `project_model_call_expected_compare` 全部通过
- 对 gitnexus-rust-core 自身跑 `--include calls`，验证 `split_last_segment` 调用解析
