# Rust Production Readiness Preflight

**日期：** 2026-05-08
**状态：** Preflight
**类型：** Rust Production Readiness Audit + Bug Fix

---

## 1. Smoke 目标

| # | 目标 | 类型 | 规模 |
|---|------|------|------|
| 1 | gitnexus-rust-core (自身) | 真实 Rust workspace（3 crates, 49 source files） | 中型 |
| 2 | fixtures/call-resolution/c1-same-module | Repo 内 fixture | 小型 |
| 3 | fixtures/call-resolution/c10-external-crate | Repo 内 fixture（含外部 crate 调用） | 小型 |

## 2. Smoke 结果

### 2.1 gitnexus-rust-core（自身）

| 指标 | 值 |
|------|-----|
| Nodes | 1278 |
| Edges | 1868 |
| Symbol nodes | 603 |
| CALLS edges | 843 |
| DESIGNATION edges | 22 |
| ACCESSES edges | 130 |
| Duplicate node IDs | 0 |
| Duplicate edge triples | 0 |
| Dangling source refs | 0 |
| Dangling target refs | **459（均为 CALLS edges）** |
| 确定性输出 | ✅（两次运行输出完全一致） |

**CALLS 分析：**
- Resolved: 843/843（100% 的 CALLS edges 有 resolved_symbol_id）
- Dangling: 459/843（54.4% 的 CALLS edges 的 target 节点不存在）
- 459 个 dangling CALLS 的 reason 分布：
  - `call-stdlib-trait-method-resolved`: 227
  - `call-external-crate-path-resolved`: 138
  - `call-receiver-type-method-resolved`: 94
- 43 个唯一 dangling target，全部为 std/core/alloc 外部符号（如 `std::string::ToString::to_string`、`std::vec::Vec::new` 等）
- 外部（std/core/alloc）symbol nodes: **0**（未被提取）

### 2.2 c1-same-module fixture

| 指标 | 值 |
|------|-----|
| Nodes | 7 |
| Edges | 6 |
| CALLS edges | 1 |
| Dangling | 0 |
| 确定性 | ✅ |

### 2.3 c10-external-crate fixture

| 指标 | 值 |
|------|-----|
| Nodes | 11 |
| Edges | 18 |
| CALLS edges | 8 |
| Dangling CALLS target | **8（100%）** |
| 确定性 | ✅ |

## 3. 根因分析

### Dangling CALLS edges（Bug）

**位置：** `crates/project-model/src/graph.rs:505-530`

`emit_graph()` 中的 CALLS edge 发射逻辑无条件地为所有已解析调用创建 edge：

```rust
let target_id = format!("symbol:{}", resolved_id);
insert_edge(..., &target_id, ...);
```

问题：`resolved_id` 可能是外部 crate symbol（如 `std::string::ToString::to_string`），但这些外部符号的 symbol node 从未被提取（symbol extraction 仅覆盖项目内源码）。因此 target node 不存在于 nodes map 中。

**对比 ACCESSES edge 发射（line 565-601）：**

```rust
// 只当类型能解析到同 crate 已知 type symbol 时才产 edge。
// stdlib 类型（如 String、Vec）在本阶段不产 edge——因为不会创建外部 symbol node，
// 避免 dangling edge。
```

ACCESSES 通过只查找同 crate symbol 来避免 dangling edge。CALLS 缺少这一保护。

**对比 AGENTS.md stop-line：**
> Graph CALLS edge must not be dangling — schema v0.2 可产 CALLS edge，但 source/target 必须指向已存在 node

当前行为违反此 stop-line。

### 其他质量门状态

| 质量门 | 状态 |
|--------|------|
| Duplicate node IDs | ✅ 0 |
| Duplicate edge triples | ✅ 0 |
| Dangling source refs | ✅ 0 |
| Dangling target（非 CALLS） | ✅ 0 |
| 确定性输出 | ✅ |
| 节点类型覆盖 | ✅ Repository/Workspace/Package/Target/SourceFile/Module/Symbol/Diagnostic |
| 边类型覆盖 | ✅ CALLS/DESIGNATION/ACCESSES/CONTAINS_PACKAGE/OWNS_SOURCE/DEFINES 等 |

## 4. 修复方案

**方案：为被 CALLS 引用的外部符号发射 minimal 外部 symbol node**

修改 `emit_graph()`，在 CALLS edge 发射之后，遍历所有 CALLS edge target，对不在 nodes map 中的 target symbol ID 创建 minimal external symbol node：

- `id`: `symbol:<external_path>`（已由 call resolution 提供）
- `label`: `symbol`
- `properties`: 仅包含 `name` 和 `isExternal: true`，无 source 位置信息

优势：
- 保留所有已解析 CALLS edges（不丢失 459 条边信息）
- 保证 endpoint integrity（graph contract）
- 对齐 AGENTS.md stop-line
- 外部符号节点为 minimal（不增加复杂度）

边界：
- 仅处理 CALLS edge target 中缺失的 symbol node
- 不扩展到 ACCESSES/DESIGNATION（这些已有自己的保护逻辑）
- 不尝试提取外部 crate 的完整符号列表

### Write set
- `crates/project-model/src/graph.rs`：在 CALLS edge 发射后添加外部 symbol node 补全逻辑
- `crates/cli/tests/project_model_graph_emit.rs`：更新 c10 test 验证 endpoint integrity

### Forbidden set
- 不修改 call resolution 逻辑
- 不修改 symbol extraction
- 不新增依赖
- 不修改 GitNexus-RC / Tool / live repo

## 5. Rust Production Readiness 判断

**当前状态：接近本地生产试用候选，但必须先修复 dangling CALLS edges bug。**

证据：
- 所有非 CALLS 的质量门均通过（0 dup, 0 dangling non-CALLS, 确定性输出）
- CALLS resolution rate 54%（1189/2203，之前 measurement），但对外部 crate 调用的 endpoint integrity 缺失
- 设计良好的 graph schema（8 node types, 11 edge types）
- 有基础的 fixture 测试覆盖（但缺少 graph contract 测试 — 后续 Priority 3）

修复 dangling CALLS edges 后状态：
- 预期所有 CALLS edges 的 source/target 节点均存在于 graph 中
- 外部 crate 调用仍以 CALLS edges 表示，但 target 指向 minimal external symbol nodes
- 质量门与 Cangjie 对齐（0 synthetic/duplicate/dangling）

Not ready for:
- 真实项目 CI/CD（缺少 graph contract 测试、缺少 multi-project smoke）
- 影响分析可用性（需要 ACCESSES/MODIFIES edge 完善）
- 用户面产品（无错误恢复、无增量分析、无缓存）

## 6. Stop-lines（重申）

- No type inference / trait solving
- No macro expansion
- No full cfg evaluator
- No cargo metadata execution
- No proc-macro / build.rs execution
- No MCP server / WebUI / HTTP / embedding
- No modification of GitNexus-RC / Tool / live repo
