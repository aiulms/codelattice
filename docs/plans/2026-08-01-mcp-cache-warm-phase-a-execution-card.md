# MCP Cache Warm 优化 — Execution Card（2026-08-01）

> **状态**: Closed（evidence-driven no-op） · **日期**: 2026-08-01 · **方案**: [2026-05-29-facade-cache-warming-extreme-optimization.md](./2026-05-29-facade-cache-warming-extreme-optimization.md)
> **目标**: 冷启动 facade cache warming ~18.5s → <3s（Phase A 先行）

---

## 1. 本轮范围

只做 **Phase A：消灭 JSON 中间层**（`GraphView::build_from_model` typed 化）。
Phase B（统一文件发现）/ C（rayon 并行）/ 4.1（typed 持久化）不在本轮，除非 Phase A 提前完成且验证充分。

## 2. Write Set

| 文件 | 变更 |
|------|------|
| `crates/cli/src/mcp_server.rs` | GraphView struct + `build_from_model` + consumer 访问路径（index 化） |
| `crates/cli/src/mcp_facade.rs` | 若引用 GraphView 结构 |
| `crates/cli/src/mcp_node_helpers.rs` | 若引用 GraphView 结构 |
| `docs/plans/2026-08-01-ai-usage-optimization-pack.md` | 追加执行结果 |
| `CHANGELOG.md` | Unreleased 追加（完成后） |

## 3. Forbidden Set（不做）

- 不改 graph edge/node **语义**（id 格式、kind 值、confidence/reason 值）
- 不做 trait solving / type inference / macro expansion / cargo metadata
- 不引入 unsafe
- 不改 `serde_json::Value` 输出契约（MCP/CLI 输出格式保持逐字节等价）
- 不删除 `GraphView::build(&Value)` —— 非 Rust 语言仍走 JSON 路径
- 不执行 Phase B/C/4.1
- 不 commit / 不 push（除非用户明确指示）

## 4. Stop-line

- 行为等价：`cargo test` 全量通过（59 suites，含 MCP regression 172+ tests）
- 确定性：自分析输出与基线 diff 无差异（除 generatedAt 等已知不稳定字段）
- 基准：每次 Phase 后重跑自分析 benchmark，记录耗时对比
- 若某一步测试失败超过 3 次：停止、回退到最近绿点、咨询
- 不允许大范围重排掩盖语义变更

## 5. 验证命令

```bash
cargo fmt --check
cargo test
scripts/codelattice-precommit-check.sh
# 基准
time target/release/codelattice analyze --root . --language rust --format json --output /tmp/base.json
# MCP 实测（initialize + tools/call project quick）
```

## 6. 基准基线（2026-08-01 实测，release build）

| 场景 | 文件数 | 冷 warm 耗时 | 结论 |
|------|--------|-------------|------|
| CodeLattice workspace（prewarm job） | 113 | **563ms** | 已极快 |
| crates/cli（project job） | 9 | ~2s（debug）/ 更小（release） | 已快 |
| open-nwe/backend（真实大项目，prewarm job） | 540 | **6.8s** | 主要瓶颈 |
| open-nwe/backend CLI analyze | 540 | **5.8s**（63,706 nodes / 88,392 edges / 84MB JSON） | engine 占绝对大头 |

## 7. 关键证据 → 决策（2026-08-01 更新）

**MCP 冷 warm（6.8s）≈ CLI analyze（5.8s）**：GraphView::build + serde_json 深拷贝在总耗时中占比已趋近于零（差额 ~1s 内）。

- Draft（2026-05-29）声称 "engine 0.58s + facade warm 18.5s"，**与 2026-08-01 实测不符**：同样 540-646 文件规模下 engine 就需 5.8s，warm 只比 engine 多 ~1s。
- 推测原因：5-29 之后引擎已发生多次优化（calls.rs 拆分/索引提取、scheduler 元数据等），或当时测量基于不同路径/未优化代码。
- **决策：Phase A（typed GraphView::build_from_model）暂停** —— 收益前提（GraphView 是瓶颈）已被证据推翻，强行重构是高风险的无效投入。此项从本轮范围移除，如需恢复需先复现 draft 的 18.5s 场景。

## 8. 新瓶颈定位（后续方向）

真实瓶颈 = **engine 串行分析管线**（540 文件 5.8s release）。对应 draft 的 **Phase C（rayon 并行化）**：

- `extract_symbols_from_files_parallel`（item.rs，rayon）已存在且 ≥8 文件时启用 ✅
- `extract_and_resolve_calls`（calls.rs）**串行**，是最大未并行项
- 需要 analysisTrace 阶段时间确认 calls/imports 占比（当前 CLI 输出未暴露 analysisTrace，需补）
- Phase C 前置验证：`CalleeIndex`/`ImportBindingTable` 只读性、tree-sitter Parser 线程安全（per-thread 创建方案已有先例）

本轮结束状态：基准完成、Phase A 按证据关闭、Phase C 作为后续候选（需新 execution card）。

## 9. Phase C Handoff

Phase A 到此关闭，不承载 Phase C 的 write set、实现或验收记录。imports/calls 并行化及 `analysisTrace` 契约修复由独立卡继续治理：

- [2026-08-02-mcp-cache-warm-phase-c-execution-card.md](./2026-08-02-mcp-cache-warm-phase-c-execution-card.md)

原 `<3s` 是严格目标；Phase C 记录的 3.1s 属 near-target，不在本卡中改写为达标。
