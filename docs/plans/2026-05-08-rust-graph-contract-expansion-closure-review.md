# Rust Graph Contract Expansion Closure Review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 3 — Rust graph contract / quality gates
**Commits：** （待 commit）

---

## 总结

本轮扩展 Rust graph contract 测试从 8 tests on 1 fixture → 23 tests on 3 fixtures，缩小与 Cangjie graph contract（24 tests on 4 fixtures）的覆盖差距。

## 修改内容

### Slice 1: imports-cross-crate fixture

**新增 `fixtures/rust/imports-cross-crate/`（compile-valid）：**
- `Cargo.toml` + `src/lib.rs`
- 通过 `use std::collections::HashMap` 引入 stdlib 类型
- 定义 `DataStore` struct + impl block（new/insert/get）+ `create_store()` 辅助函数
- 使用 `Vec::new()`, `HashMap::new()`, `String::from()`, `key.clone()` 触发外部 crate 调用

**Graph 产出（14 nodes, 22 edges, 8 edge types）：**
- 4 external symbol nodes（isExternal=true）：`Vec::new`, `HashMap::new`, `String::from`, `Clone::clone`
- 7 CALLS edges（含 external-crate-path-resolved ×3, stdlib-trait-method-resolved ×1, associated-fn-resolved ×1, method-name-resolved ×2）
- 2 ACCESSES edges（同 crate struct 类型注解）
- 1 DESIGNATION edge（impl DataStore → DataStore）

### Slice 2: multi-module fixture

**新增 `fixtures/rust/multi-module/`（compile-valid）：**
- `Cargo.toml` + `src/lib.rs` + `src/utils.rs`
- lib.rs 声明 `pub mod utils;` 并通过 `crate::utils::double_value()` 和 `crate::utils::format_result()` 跨模块调用
- utils.rs 定义 `double_value()` 和 `format_result()` 函数

**Graph 产出（10 nodes, 12 edges, 5 edge types）：**
- 2 source files（lib.rs + utils.rs）
- 5 DEFINES edges（2 per source file）
- 3 CALLS edges（2 call-crate-path-resolved + 1 call-same-module-resolved）
- 2 OWNS_SOURCE edges

### Slice 3: 扩增 contract tests（+15 new tests）

**imports-cross-crate（8 tests）：**
1. `quality_gates` — 0 dup, 0 dangling, deterministic
2. `node_kind_set` — 验证 5 种核心 node kind
3. `edge_kind_set` — 验证 7 种核心 edge kind（含 CALLS ≥ 4）
4. `known_symbols` — 验证 DataStore/new/insert/get/create_store 存在
5. `known_calls_edges` — 验证 3 条 external crate CALLS edges
6. `external_symbol_nodes` — 验证 4 external symbol nodes 存在 + CALLS endpoint integrity
7. `calls_endpoint_integrity` — 所有 CALLS source/target 存在
8. `known_designation_edge` — impl DataStore → DataStore

**multi-module（7 tests）：**
9. `quality_gates` — 0 dup, 0 dangling, deterministic
10. `node_kind_set` — 验证 2+ source files, 4+ symbols
11. `edge_kind_set` — 验证 2+ OWNS_SOURCE, 3+ CALLS
12. `known_symbols` — 验证 process_data/run_pipeline/double_value/format_result
13. `known_defines_edges` — 验证 4 条跨文件 DEFINES edges
14. `known_calls_edges` — 验证 3 条 CALLS edges（含 crate:: 路径调用）
15. `calls_endpoint_integrity` — 所有 CALLS source/target 存在

## 合约覆盖对比

| 指标 | Before | After | Cangjie |
|------|--------|-------|---------|
| Contract tests | 8 | 23 | 24 |
| Contract fixtures | 1 | 3 | 4 |
| External symbol nodes covered | 0 | 4 | — |
| Cross-module crate:: path | 0 | 2 | — |
| Multi-file DEFINES | 0 | 4 | — |
| ACCESSES edges fixture | portable-smoke | +imports-cross-crate | — |

## 验证

- `cargo fmt --check` ✅ clean
- `git diff --check` ✅ clean
- `cargo test` ✅ 全部通过（no-feature）
- `cargo test --features tree-sitter-cangjie` ✅ 全部通过
- `cargo test --features tree-sitter-cangjie --test cangjie_inspect -- --nocapture` ✅ 18/18 pass
- `cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture` ✅ 24/24 pass
- `cargo test --features tree-sitter-cangjie --test project_model_graph_contract -- --nocapture` ✅ 23/23 pass
- 新增 fixture 编译验证 ✅

## Stop-lines 合规

- ✅ 未做 type inference / trait solving
- ✅ 未做 macro expansion
- ✅ 未做 full cfg evaluator
- ✅ 未新增依赖
- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未做 destructive git 操作

## 残留限制

1. ACCESSES edges 仍限于同 crate type（stdlib 类型不产 ACCESSES edge）
2. 外部 symbol node 仅由 CALLS edge target 触发（type annotation 引用不创建外部 node）
3. Rust graph contract 仍缺 1 个 fixture vs Cangjie（4 fixtures）

## 下一轮 Opening

**Priority 2 续 — Rust CALLS resolution quality：**
- `split_last_segment` 歧义修复（wildcard import 源模块感知优先，5 calls）
- `crate::` 多段路径分类修复（associated-function 误分类为 qualified-path，2 calls）
- 扩张 method call resolution（stdlib type methods 扩展）

**Priority 3 续 — 可考虑第 4 个 Rust contract fixture（如 `module-hierarchy`）**
