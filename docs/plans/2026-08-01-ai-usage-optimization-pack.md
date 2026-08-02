# AI 使用角度优化 Pack（2026-08-01）

> **状态**: Executed（AI 使用优化）+ P0 Closed（Phase A no-op、Phase C near-target）
> **日期**: 2026-08-01
> **驱动**: AI 使用角度审计报告（MCP 实现 / 文档资产 / 缓存性能三视角探索）

---

## 0. 背景

对 CodeLattice 从 AI 使用角度做了三路审计：

1. **MCP 实现**（`crates/cli/src/mcp_server.rs`，32,909 行单体）：50 个工具实际注册（文档/header 声称 49 —— 漂移）；compact 分层控制、结构化错误、busy envelope、tokenBudget 已是成熟设计。
2. **文档资产**（docs/guides + docs/mcp + docs/architecture）：`ai-usage-guide.md` 是最强资产但未从 guides README 链接；workflow mode 词汇在三个文档间不一致；`workflow-presets.md` 引用默认工具面下不可见的低层工具；`known-limitations.md` 是空骨架。
3. **缓存/性能**：暖缓存 94% 时间花在 GraphView::build + serde_json 克隆（已有 2026-05-29 draft 方案未落地）；缓存键 `{root, language, strict}` 粒度粗；scheduler `plan_only=true` 只规划不执行；engine `ArtifactCache` 生产侧基本未接线。

---

## 1. 本轮已落地（低风险、高确定性）

### 1.1 代码修复（2 处，crates/cli/src/mcp_server.rs）

| 项 | 变更 | 理由 |
|----|------|------|
| serverInfo.version 硬编码 | `"0.17.0-beta.1"` → `env!("CARGO_PKG_VERSION")`（18682 行） | 编译期注入，杜绝与 workspace.package.version 漂移；AI 侧版本兼容判断读到真实版本 |
| header 工具计数 | `Provides 49 tools` → `Provides 50 tools`（4 行），补 `v0.31 (unreleased): codelattice_complexity_hotspots`（22 行后） | 实际注册 50 个工具，文档计数漂移 |

注：`CODELATTICE_CACHE_VERSION` 常量（919 行）保留字面量 —— 它是缓存失效机制的一部分，与展示用 serverInfo.version 语义不同，本轮不改。

### 1.2 文档计数修正（49 → 50，仅当前状态文档，不动 CHANGELOG/release 历史快照）

- `README.md`：4 处
- `docs/guides/ai-mcp-tool-guide.md`：2 处
- `docs/mcp/ai-usage-guide.md`：1 处（章节标题）
- `docs/architecture/mcp-local-client-setup.md`：2 处
- `docs/architecture/mcp-v0-contract.md`：1 处

### 1.3 文档一致性与补链

| 文档 | 变更 |
|------|------|
| `docs/guides/README.md` | 补链 `ai-mcp-tool-guide.md`、`../mcp/ai-usage-guide.md`、`../architecture/mcp-v0-contract.md`；表格改为中文用途说明 |
| `docs/decisions/known-limitations.md` | 空骨架 → 填实：引擎级 stop-lines、语言级边界表、MCP/AI 工具面局限、降级行为、来源引用 |
| `docs/guides/workflow-presets.md` | 新增 **Facade Equivalence** 章节：15 个低层工具 → 默认 6-tool facade 等价映射表（基于代码权威 mode enum 验证） |
| `docs/guides/ai-mcp-tool-guide.md` | workflow 典型模式行更新为完整 17 mode（与 `mcp_server.rs:13467` enum 一致），表格加 schema 为准脚注 |
| `docs/mcp/ai-usage-guide.md` | workflow mode 行更新为完整 17 mode，加"常用子集，schema 为准"脚注 |

### 1.4 验证状态

- 脚本侧无需改动：`linux-source-build-smoke.sh`（≥51）、`promote-to-local-tool.sh`（≥50）已是动态断言。
- `cargo fmt --check` / `cargo test` / `scripts/codelattice-precommit-check.sh`：见文末执行记录（待填）。

### 1.5 第二轮追加（2026-08-01）

| 项 | 变更 | 判定 |
|----|------|------|
| Cookbook 错误恢复提示词 | `ai-prompt-cookbook.md` 新增 §14-18：mcp_server_busy / tool_not_in_ai_toolset（含 facade 等价映射）/ needs_input / 符号消歧 / 缓存 stale 与 jobNotReady | 完成 |
| tools/list token 成本 | 实测：AI toolset（6 工具）≈15KB / ~4.3K tokens；full（50 工具）≈94KB / ~27K tokens | **无需裁剪** —— AI toolset 体量可接受；full 模式已有文档警告且非日常路径。6-tool facade 设计有效性得到量化验证 |

---

## 2. P0 缓存里程碑后续记录（由独立 execution card 执行）

### 2.1 暖缓存路径优化（旧假设 18.5s，严格目标 <3s）

原方案见 [2026-05-29-facade-cache-warming-extreme-optimization.md](./2026-05-29-facade-cache-warming-extreme-optimization.md)。2026-08-01 实测未复现 18.5s 瓶颈，因此执行结果按新证据调整：

- Phase A：关闭为 evidence-driven no-op；GraphView 差额约 1s 内，不支持高风险 typed 重构。
- Phase C：独立执行 imports/calls per-file rayon 并行化；历史实测 engine 4.4x、MCP 冷 warm 6.8s→3.1s（2.2x）。
- 严格 `<3s` 未达到，3.1s 记录为 near-target；Phase B 与 Phase 4.1 保持低优先级 backlog。
- 治理记录：[Phase A closure](./2026-08-01-mcp-cache-warm-phase-a-execution-card.md) / [Phase C execution card](./2026-08-02-mcp-cache-warm-phase-c-execution-card.md)。

### 2.2 scheduler 增量落地（plan_only → 执行）

`analysis-scheduler/src/lib.rs` 目前 `plan_only=true`（441 行），只产出 `incrementalPlan` 元数据。AI 工作流"改代码 → 再查图谱"循环中，任何文件变动都会使整个项目级 artifact 失效（缓存键 `{root, language, strict}`，`mcp_server.rs:888`）。后续应把 dirty-file 计划接到执行路径（文件级部分重建），或至少将缓存键升级为 scheduler fingerprint 感知。

### 2.3 遗留技术债（低优先级）

- engine `ArtifactCache`（`analysis-engine/src/cache.rs`）生产侧只有 store 无 get；磁盘键不含 content_hash（未来接线会踩 stale 覆盖）—— 接线或删除二选一
- `mcp_server.rs` 32,909 行单体，后续按工具组拆分
- envelope schemaVersion 字符串散落，考虑集中注册表

---

## 3. 2026-08-01 AI 文档审计轮次边界（历史记录）

- 不改 CHANGELOG.md / release notes / smoke-matrix（历史快照，描述当时 49 工具状态）
- 不动 `CODELATTICE_CACHE_VERSION`（缓存失效机制语义）
- 不执行 P0 缓存重构（需独立 preflight + 基准测量，已有 draft 但未批准执行）
- 不 commit / 不 push（等待用户指示）

注：P0 后续已在独立 preflight / execution card 下执行，不改变上述原轮次边界的历史含义。

---

## 4. 验证执行记录

- `cargo fmt --check`：PASS（0 diff）
- `cargo check -p gitnexus-rust-core-cli`：PASS（42 个既有 warning，非本轮引入）
- `cargo test`：59/59 test suite PASS，0 failed
- MCP initialize 实测：`serverInfo.version=0.17.0-beta.1`（env! 注入生效）、`toolCount(ai)=6`、`fullToolCount=50`、tools/list 返回 6 个 AI 工具
- `scripts/codelattice-precommit-check.sh`：PASS（MCP regression + detect-changes smoke 17/17, FAIL 0）
- detect-changes 风险等级：critical（因修改 mcp_server.rs 核心文件，跨项目影响 16 个；已审查变更内容，9 个 tracked 文件均为预期改动）—— 未 commit / 未 push

**第二轮（2026-08-01 追加）：**
- `git diff --check`：PASS
- 本轮仅文档改动（cookbook + pack），无代码变更，无需重跑 cargo；上一轮全量验证仍有效
- tools/list token 实测：ai=4.3K / full=27K tokens（见 §1.5 判定）
