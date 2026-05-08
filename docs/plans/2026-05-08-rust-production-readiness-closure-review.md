# Rust Production Readiness Smoke + CALLS Endpoint Integrity Fix — Closure Review

**日期：** 2026-05-08
**状态：** Closure Review
**类型：** Production Readiness Audit + Bug Fix
**父文档：** [Preflight](2026-05-08-rust-production-readiness-preflight.md)

---

## 总结

对 Rust 线进行了首次生产就绪 smoke audit，发现并修复了 CALLS edge 外部符号 dangling 问题（459 条 dangling edges → 0）。修复方案：为被 CALLS 引用但未被 symbol extraction 覆盖的外部 crate symbol 发射 minimal 外部 symbol node。

## 变更

### `crates/project-model/src/graph.rs`（+22 行）

在 CALLS edge 发射之后，新增「外部 symbol node 补全」逻辑：

1. 遍历所有 CALLS edges，收集 target 不在 nodes map 中的 symbol ID
2. 对每个缺失的外部 symbol ID，创建 minimal GraphNode（id + label="symbol" + `isExternal: true`）
3. Symbol name 从完整路径（如 `std::string::ToString::to_string`）提取最后一段

**设计决策：**
- 仅在 graph emission 阶段补全，不修改 call resolution 或 symbol extraction 管道
- 外部 symbol node 仅包含 id/name/isExternal，无 source 位置信息
- 对齐 ACCESSES edge 的保护逻辑（同 crate only），但 CALLS 采用「补全」而非「抑制」策略（保留已解析的调用信息）

### `crates/cli/tests/project_model_graph_emit.rs`（~20 行变更）

`test_graph_c10_external_crate_produces_calls_edges_for_stdlib` 测试增强：
- 新增 endpoint integrity 验证：每条 CALLS edge 的 source/target 节点必须存在
- 新增外部 symbol node isExternal 标记验证
- 保持原有的 callEdgeCount > 0 断言

### `docs/plans/2026-05-08-rust-production-readiness-preflight.md`（新建）

Rust 生产就绪 audit 文档：smoke 结果、根因分析、修复方案、质量门状态。

## 修复效果

| 指标 | 修复前 | 修复后 |
|------|--------|--------|
| Nodes | 1278 | 1322（+43 external symbol nodes） |
| Edges | 1868 | 1868（不变） |
| CALLS edges | 843 | 843（全部保留） |
| Dangling CALLS target | 459 | 0 |
| Dangling source | 0 | 0 |
| 确定性输出 | ✅ | ✅ |
| Duplicate nodes/edges | 0 | 0 |

## 完整性验证

- `cargo fmt --check`: clean
- `git diff --check`: clean
- `cargo test`: 全部通过
- `cargo test --features tree-sitter-cangjie`: 全部通过
- `project_model_graph_emit` tests: 10/10 pass（含更新后的 c10 endpoint integrity test）
- `cangjie_inspect` tests: 18/18 pass
- `multi_project_smoke` (production): 4/4 pass（nodes: 3471, edges: 9746, synth=0, dang=(0,0)）
- 确定性：两次运行输出完全一致

## Stop-lines 合规

- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未新增依赖
- ✅ 未做 destructive git 操作
- ✅ 未修改 call resolution 或 symbol extraction 管道
- ✅ 未扩展 WebUI/MCP/HTTP/embedding
- ✅ 外部 symbol node 为 minimal（不涉及 type inference / trait solving / macro expansion）

## Rust 当前 Readiness 判断

**状态：CALLS endpoint integrity 已修复，整体接近本地生产试用候选。**

- 所有质量门通过（0 dup, 0 dangling, 确定性输出）
- CALLS 54% resolution rate 不变，但所有边现在有 endpoint 保证
- 下一步：Rust graph contract tests（Priority 3）、ACCESSES/MODIFIES edge 扩展、真实项目 multi-project smoke

## Cangjie 当前 Readiness 判断

**状态：持续稳定，4/4 production targets 全部通过。**

- 无需本轮变更
- 继续维护模式（Priority 4）
