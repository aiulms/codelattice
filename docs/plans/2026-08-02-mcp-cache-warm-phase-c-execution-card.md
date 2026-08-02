# MCP Cache Warm Phase C — Execution Card（2026-08-02）

> **状态**: Closed
> **前置记录**: [2026-08-01-mcp-cache-warm-phase-a-execution-card.md](./2026-08-01-mcp-cache-warm-phase-a-execution-card.md)
> **目标**: 闭环 per-file imports/calls rayon 并行化，修复附带 tracing 契约漂移，并完成独立治理记录。

## 1. Preflight 结论

2026-08-01 基准推翻了“GraphView JSON 中间层是主要瓶颈”的旧假设：大仓冷 warm 的主要耗时位于 import/call resolution。Phase A 因缺乏收益证据关闭为 no-op；Phase C 是新的独立执行 slice，不继承 Phase A 的 write set。

CodeLattice impact review（2026-08-02）：

| 目标 | 风险 | 主要影响面 |
|---|---|---|
| `LanguageAnalysisResult` | MEDIUM | `print_analyze_result`、`run`、CLI 公共输出 |
| `filter_analyze_profile` | MEDIUM | full/compact profile 消费路径 |
| `extract_and_resolve_imports` | MEDIUM | `inspect_project_model_with_options` 及公开封装 |
| `extract_and_resolve_calls` | MEDIUM | `inspect_project_model_with_options` 及公开封装 |

本轮属于 **CodeLattice landed reality 的调度/输出包装变更**，不改变 Rust compiler truth、Cargo truth 或 graph policy。

## 2. Write Set

| 文件 | 允许变更 |
|---|---|
| `crates/project-model/src/imports.rs` | per-file rayon flatten 边界优化，不改 resolver |
| `crates/project-model/src/calls.rs` | per-file rayon flatten 边界优化，不改 resolver |
| `crates/cli/src/unified_types.rs` | `analysisTrace` 可选序列化策略 |
| `crates/cli/src/lib.rs` | compact profile 仅在 trace 存在时透出 |
| `crates/cli/tests/productization_commands.rs` | Rust trace presence / non-Rust trace absence 回归测试 |
| `docs/architecture/unified-output-contract.md` | 可选 `analysisTrace` 字段契约 |
| `docs/architecture/consumer-contract.md` | 消费侧兼容性说明 |
| `docs/plans/2026-08-01-mcp-cache-warm-phase-a-execution-card.md` | Phase A closure 与 Phase C handoff，不再承载 Phase C 实施记录 |
| `docs/plans/2026-08-01-ai-usage-optimization-pack.md` | P0 状态同步 |
| `CHANGELOG.md` | 已有性能条目措辞校正 |
| `docs/plans/2026-08-02-p0-cache-optimization-closure/*` | 计划、findings、progress |
| 本文件 | 执行与 closure 记录 |

## 3. Forbidden Set

- 不改变 IMPORTS/CALLS node/edge、id、kind、confidence、reason、dedupe 或 no-edge 语义。
- 不做 type inference、trait solving、macro expansion、cfg evaluator、`cargo metadata` 或 external crate API resolution。
- 不执行 Phase B 文件发现统一或 Phase 4.1 typed graph 持久化。
- 不修改 open-nwe、cangjie 等 live repo，也不对其重新执行 production analyze。
- 不修改或清理其他未提交用户改动；不 stage、commit、push。
- 不用大范围重排掩盖语义变更。

## 4. TDD 与验证门

1. 先新增产品化测试并确认 RED：
   - Rust full/compact 输出包含 object 类型 `analysisTrace`。
   - 非 Rust full/compact 输出不包含 `analysisTrace` key。
2. 最小实现转 GREEN。
3. imports/calls golden fixtures、absence assertions、endpoint integrity 和 deterministic tests 必须通过。
4. 三次本仓分析去除 `analyzedAt` / `analysisTrace` 后必须逐字节一致。
5. `cargo fmt --check`、`git diff --check`、`cargo test` 必须通过。
6. `scripts/codelattice-precommit-check.sh` 必须执行；若仍因混合工作区报告 high/critical，closure 必须拆分说明 P0 目标文件风险与无关改动风险。

## 5. 性能验收

- 已记录的 open-nwe/backend 数据作为历史实测证据保留，不在本轮重跑。
- 本轮使用允许的本仓 root 做 release 多轮对照；`flat_map_iter` 候选因无收益且原始中位数回退而拒绝，生产代码保持原 `flat_map`。
- 原 `<3s` 是 aspirational strict target；当前记录 3.1s 属 near-target。除非获得可重复的多轮数据，否则不得宣称严格达到 `<3s`。
- 主要交付判据为：并行化收益成立、行为等价、契约兼容、治理闭环。

## 6. Stop-line

- 任一 graph fixture 出现语义差异立即停止性能清理并回退该清理。
- 非 Rust 输出仍出现 `analysisTrace: null` 时不得关闭本卡。
- 未获得可重复证据时不得修改历史数字以制造达标结论。
- native detect-changes 报告 high/critical 时不得 commit/push；本轮本来也不授权 commit/push。

## 7. Closure Review

### 7.1 交付结论

- 保留已验证的 imports/calls per-file Rayon 并行化及 merge 后稳定排序；不改变 resolver、confidence、reason、dedupe、no-edge 或 graph endpoint 语义。
- `analysisTrace` 契约按 TDD 从 RED 转 GREEN：Rust full/compact 输出对象，非 Rust full/compact 省略字段。
- `flat_map_iter` follow-up 在允许的本仓 release proxy 上无收益且原始中位数回退，已完整恢复为 `flat_map`；没有把猜测性微优化留在生产代码中。
- 历史 open-nwe/backend 数据保留为 4.4x engine / 2.2x MCP 证据；MCP 3.1s 是 near-target，未宣称严格满足 `<3s`。

### 7.2 验证结果

| Gate | 结果 |
|---|---|
| TDD RED | Shell full profile 因旧 `analysisTrace: null` 按预期失败 |
| TDD GREEN | 2/2 trace contract tests PASS |
| 目标回归 | productization + IMPORTS/CALLS + graph contract/emit 共 106 tests PASS |
| 全量测试 | `cargo test` PASS，0 failed |
| 格式与差异 | `cargo fmt --check`、`git diff --check` PASS |
| 确定性 | 三次本仓 full analyze 去除 `analyzedAt` / `analysisTrace` 后 SHA-256 均为 `69ad79e63af164bdd29d63472464025acaf8d15a74217fcebf178cdf6bdd4bf3` |
| 回退后 release proxy | 五次中位数：project-model `27/6/12ms`，仓库根 `228/21/38ms`（total/import/call） |
| Native precommit | productization、338 MCP tests、concurrency、17/17 detect smoke 全部 PASS |

### 7.3 风险拆分

预编辑四个目标符号的 native impact 均为 MEDIUM，无 high/critical。最终 workspace detect-changes 报告 `critical`，来源是混合工作区整体：18 个 tracked 文件、11 个 untracked 文件、42 个 unknown hunks、16 个受影响项目和 2 个 unsupported-boundary hits。该 workspace 结论不能归因于本 P0 slice；按 stop-line 本轮不 stage、commit 或 push。

## 8. Publication Addendum（2026-08-02）

上述 no-stage/commit/push 边界适用于实现与 closure 轮次。用户在收到 mixed-worktree `critical` 警告后，另行明确授权执行 scoped review、精确暂存、commit 与 push。发布阶段采用新的边界：

- P0 实现/契约/Phase A-Phase C 治理文件单独成 commit；AI 工具面一致性改动另成 commit；closure 过程记录最后成 commit。
- `.cursor/`、`.omo/` 以及任何未列入精确路径集的内容不得暂存。
- 每个 commit 前核验 staged path、staged diff 与 `git diff --cached --check`；只 push `gitcode/master`。
- 用户授权不改变历史性能结论、stop-lines 或 live-repository 禁令。
