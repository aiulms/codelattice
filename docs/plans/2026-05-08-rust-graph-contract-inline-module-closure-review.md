# Closure Review: 第5个 Rust Graph Contract Fixture — inline-module

日期：2026-05-08
状态：完成
关联 Preflight：`docs/plans/2026-05-08-rust-graph-contract-inline-module-preflight.md`

## Landed Reality

### 新增内容

1. **Fixture `fixtures/rust/inline-module/`**（compile-valid，1 source file）
   - 顶层函数 `root_fn()` + inline module `inner` + 嵌套 inline module `inner::nested`
   - 4 个 CALLS 检测（3 self/super/crate + 1 to_string）
   - self:: 和 super:: 调用被检测但 unresolved（已知 inline module path flat 限制）
   - crate:: 从嵌套 inline module 调用成功解析

2. **7 个 contract tests** 在 `project_model_graph_contract.rs`
   - quality_gates: 0 dup, 0 dangling, deterministic
   - node_kind_set: 验证 5 种节点类型 + HAS_PARENT 边
   - edge_kind_set: 验证 6 种边类型 + HAS_PARENT ≥ 6
   - known_symbols: 8 个预期 symbol ID（含 2 个 module symbol）
   - known_defines_edges: 5 条 DEFINES 边
   - known_calls_edges: 1 条 CALLS 边（crate:: 路径）
   - calls_endpoint_integrity: 所有 CALLS source/target 存在

### Graph 产出统计

| Metric | Value |
|--------|-------|
| Nodes | 12 |
| Edges | 18 |
| Edge types | 6（CONTAINS_PACKAGE, HAS_TARGET, OWNS_SOURCE, DEFINES, HAS_PARENT, CALLS） |
| Symbol count | 8（含 2 个 module symbol） |
| CALLS edges | 1（crate:: 路径，已解析） |
| HAS_PARENT edges | 6 |
| Duplicate | 0 |
| Dangling | 0 |
| Deterministic | yes |

### 验证结果

- `cargo fmt --check`: ✅ clean
- `git diff --check`: ✅ clean
- `cargo test`（no-feature）: ✅ 全部通过
- `project_model_graph_contract`: ✅ 37/37 pass（30 existing + 7 new）
- Cangjie production gate: 未被触碰

### 已知限制（已记录）

- self:: 和 super:: 调用在 inline module 内仍 unresolved
- 根因：modulePath flat limitation（Call site modulePath 使用文件级路径，不区分 inline module 内部）
- 已在 RISK_LEDGER.md 记录，不做本文档修复

### Stop-line 合规

- No new dependencies ✅
- No GitNexus-RC / Tool / live repo modification ✅
- No destructive git ✅
- No macro expansion / type inference / trait solving ✅
