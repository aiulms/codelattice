# Rust Graph Contract Tests — Closure Review

**日期：** 2026-05-08
**状态：** Closure Review
**类型：** Rust Graph Contract / Quality Gates
**父文档：** [Rust Production Readiness Preflight](2026-05-08-rust-production-readiness-preflight.md)

---

## 总结

仿照 Cangjie `graph_contract.rs` 模式，为 Rust 线创建了 portable smoke fixture 和 8 个 graph contract regression tests。验证节点/边类型覆盖、已知 symbol ID、已知 edge triple、质量门（0 dup/0 dangling/确定性输出）。

## 变更

### `fixtures/rust/portable-smoke/`（新建，3 文件）

```
fixtures/rust/portable-smoke/
  Cargo.toml    — package "portable-smoke", edition 2021
  src/lib.rs    — Calculator struct + impl block + free functions (add, multiply, create_calculator)
  src/main.rs   — 跨 target 调用 lib 类型和函数
```

Graph 产出：16 nodes, 25 edges

节点类型覆盖：repository, package, target (2), source-file (2), symbol (9), diagnostic
边类型覆盖：CONTAINS_PACKAGE, HAS_TARGET (2), OWNS_SOURCE (2), DEFINES (9), CALLS (5), DESIGNATION (1), ACCESSES (2), HAS_PARENT (3)

质量门：0 duplicate nodes, 0 duplicate edges, 0 dangling source, 0 dangling target, 确定性输出

### `crates/cli/tests/project_model_graph_contract.rs`（新建，8 tests）

| 测试 | 验证内容 |
|------|---------|
| `rust_graph_contract_portable_smoke_quality_gates` | 0 dup, 0 dangling, 确定性 |
| `rust_graph_contract_portable_smoke_node_kind_set` | 5 种核心节点类型存在 + ≥2 source files |
| `rust_graph_contract_portable_smoke_edge_kind_set` | 7 种核心边类型存在 + ≥2 CALLS edges |
| `rust_graph_contract_portable_smoke_known_symbols` | 5 个已知 symbol ID 存在 |
| `rust_graph_contract_portable_smoke_known_defines_edges` | 5 条已知 DEFINES edge |
| `rust_graph_contract_portable_smoke_known_calls_edges` | 2 条已知 CALLS edge（main→add, main→multiply） |
| `rust_graph_contract_portable_smoke_known_designation_edge` | impl Calculator → Calculator DESIGNATION edge |
| `rust_graph_contract_portable_smoke_calls_endpoint_integrity` | 所有 CALLS source/target 必须是存在的 symbol node |

设计策略：
- 使用 CLI binary 通过 `project-model inspect --include graph --include calls --include symbols` 获取 graph JSON
- 按 HashSet membership 做语义断言（非 sort-order binding）
- 不存 JSON snapshot（golden 会因无关变更破裂）
- 两次运行验证确定性

## 设计决策

- **CLI 集成测试模式**：使用 CLI binary 而非 library API，与现有 `project_model_graph_emit.rs` 测试模式一致
- **单一 fixture**：portable-smoke 已足够覆盖所有核心节点/边类型，不需要多个 fixture（如 Cangjie 的 4 个 fixture）
- **无 external crate 调用**：portable-smoke fixture 刻意不包含外部 crate 调用，避免依赖外部 symbol node 补全逻辑（那部分由 c10 test 覆盖）

## 完整性验证

- `cargo fmt --check`: clean
- `git diff --check`: clean
- `cargo test`: 93+ lib + 所有集成测试 pass
- `cargo test --features tree-sitter-cangjie`: 112+ lib + 所有集成测试 pass
- `project_model_graph_contract`: 8/8 pass
- `project_model_graph_emit`: 10/10 pass
- `cangjie_inspect`: 18/18 pass
- `multi_project_smoke` (production): 4/4 pass

## Stop-lines 合规

- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未新增依赖
- ✅ 未做 destructive git 操作
- ✅ fixture 为 read-only（static-analysis，不需要编译）
- ✅ 未扩展 WebUI/MCP/HTTP/embedding

## Rust 当前 Readiness 判断

**状态：graph contract foundation 就位。与 Cangjie 的对齐度显著提升。**

| 维度 | Cangjie | Rust（本轮前） | Rust（本轮后） |
|------|---------|---------------|---------------|
| Graph contract tests | 24 tests, 4 fixtures | 0 | 8 tests, 1 fixture |
| Quality gates | 0 dup/0 dangling/0 synth/deterministic | Only partially verified | All verified |
| CALLS endpoint integrity | ✅ (via synthetic fallback) | ❌ (459 dangling) | ✅ (external node 补全) |
| Portable smoke fixture | ✅ | ❌ | ✅ |
| QUALITY.md | ✅ | ❌ | 下一步 |
| Multi-project smoke | ✅ 4 targets | ❌ | 下一步 |
