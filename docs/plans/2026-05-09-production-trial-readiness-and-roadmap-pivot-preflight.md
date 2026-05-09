# Production Trial Readiness and Roadmap Pivot Preflight

> **日期：** 2026-05-09
> **状态：** Preflight / 路线收束初稿
> **目的：** 定义第一版 production trial 的最低可用标准，并确认项目主线从“复刻某个现有工具”收束为“独立 Rust/Cangjie 本地代码上下文核心”。
> **Stop-line：** 本文不启动新功能、不改 runtime、不改 CLI/schema、不做最终改名、不承诺 UI/Web/MCP。

---

## 一、为什么需要这份文档

当前项目已经具备 Rust / Cangjie 双语言分析核心、统一 CLI、质量门、fixtures、真实项目 smoke 和本地构建脚本。它已经不是早期空想阶段。

但最近路线判断发生了变化：

1. 代码图谱 + AI 上下文工具不是单一项目独有路线，外部已有多种相近实现和论文方向。
2. 我们认可“静态分析先产出事实，AI 再消费事实”的技术路线，但不需要继续完全按某个项目的 UI/Web/MCP 形态复刻。
3. 短期目标应该从“继续扩产品面”转成“把 Rust + Cangjie 两门语言做到可生产试用”。
4. UI、Web、多语言大覆盖、MCP server 都可以后置；它们不是第一版 production trial 的必要条件。

所以这份文档回答的不是“还能做什么”，而是：

- 什么算第一版可投入生产试用？
- 哪些能力必须稳定？
- 哪些能力短期不做？
- 当前主线是否需要调整？
- 工作量怎么控制，避免再开大坑？

---

## 二、路线转向是否和当前主线冲突

结论：**不冲突，但需要改叙事和优先级。**

原主线中已经完成的工作仍然有价值：

- Rust / Cangjie 分析能力
- graph 输出
- quality gates
- fixtures / regression tests
- build / smoke 脚本
- consumer format 兼容性验证

需要调整的是“目标解释”：

| 原先倾向 | 调整后 |
----------|--------|
| 复刻 GitNexus 形态 | 独立本地代码上下文核心 |
| bridge / adapter 优先 | 输出协议稳定 + AI 消费最小接口优先 |
| 未来接 UI/Web | UI/Web 后置，不进第一版 production trial |
| 尽量靠近上游产品形态 | 吸收同类产品共性思路，但保留自身路线 |
| 多语言想象空间较大 | 第一阶段只稳 Rust + Cangjie |

当前主线不是被推翻，而是收束成：

> 先做一个可在真实 Rust / Cangjie 项目中稳定运行、能给 AI 提供可验证结构化上下文的本地分析核心。

---

## 三、什么叫“第一版可投入生产”

这里的“生产”不是正式商业化，不是 v1.0，也不是功能全。

更准确的定义是：

> **Alpha production trial**：可以在真实项目上作为 AI 辅助开发前置分析工具使用；输出稳定、质量门可靠、失败模式清楚、已知边界诚实，不要求覆盖完整语言语义。

必须满足：

1. 能在真实 Rust / Cangjie 项目上稳定运行。
2. 输出 JSON 字段稳定，至少在 alpha trial 周期内不随意破坏。
3. graph 不产生明显坏数据：duplicate、dangling、非 deterministic、synthetic 泄露等由 quality gate 拦住。
4. 无法确定的语义必须 no-edge 或 low-confidence，不假装完整解析。
5. AI 可以拿到 summary / quality / graph / diagnostics / key risks，不需要自己全仓猜。
6. README 说清楚当前支持什么、不支持什么、如何验证。
7. 有一套固定 smoke targets 和验收命令。

不要求：

- 完整 type inference。
- 完整 trait solving。
- macro expansion。
- proc-macro / build.rs 执行。
- 完整 cfg evaluator。
- 任意第三方 crate API 深度解析。
- Cangjie 完整 method dispatch / interface solving。
- UI / Web / MCP。
- 支持几十门语言。

---

## 四、必须稳定的命令

第一版 production trial 只冻结最小 CLI 面：

| 命令 | 必须稳定的点 | 说明 |
|------|--------------|------|
| `analyze` | 输入参数、JSON envelope、quality gate 集成、exit code | 核心入口，供 AI / script 消费 |
| `analyze --strict` | 质量门失败时 non-zero exit | CI / smoke 可用 |
| `quality` | gate 列表、JSON result、exit code 0/1/2 | 独立质量检查 |
| `summary` | 轻量统计摘要 | 适合 AI 快速判断项目规模和入口 |
| `scripts/build.sh` | release build 可重复 | 本地使用门槛 |
| `scripts/smoke.sh` | quick/full smoke 可重复 | production trial 验收入口 |

暂不冻结：

- experimental / internal 子命令。
- 旧命名的 consumer/bridge 兼容格式。
- future MCP / HTTP / UI API。

建议下一步：

- 给中性输出格式起名，例如 `context-graph-v0` 或 `graph-v0`。
- 保留旧格式名作为 internal compatibility alias，但 README 默认不再使用旧名。

---

## 五、必须冻结的输出字段

Production trial 不要求 schema v1 完全定稿，但需要冻结一组 alpha stable 字段。

### 5.1 Analyze JSON envelope

建议冻结：

- `language`
- `root`
- `schemaVersion`
- `summary`
- `qualityGates`
- `graph`

建议补充或明确：

- `schemaName`
- `toolVersion`
- `generatedAt` 是否稳定参与比较
- `warnings`
- `capabilities`

### 5.2 Graph summary

建议冻结：

- node count
- edge count
- source file count
- symbol count
- package count
- diagnostic count
- call edge count

### 5.3 Quality gate result

建议冻结：

- `gateName`
- `passed`
- `detail`
- 未来可加 `severity`，但不要破坏现有字段。

### 5.4 Graph node / edge 最小合同

必须稳定：

- node id
- node kind / label
- node properties 基础字段
- edge source
- edge target
- edge kind
- edge confidence
- edge reason

原则：

- 不确定关系必须有低置信度或不出边。
- CALLS edge 不允许 dangling。
- 输出必须 deterministic。
- graph contract tests 必须覆盖稳定字段。

---

## 六、真实项目 smoke 门槛

Production trial 不能只靠 fixture。

建议固定三层 smoke：

### Tier 1：仓库内 fixture

必须 always pass：

- Rust graph contract fixtures
- Rust call comparison fixtures
- Rust symbol/import comparison fixtures
- Cangjie graph contract fixtures
- Cangjie inspect fixtures
- Cangjie multi-project fixtures

### Tier 2：本机真实项目只读 smoke

建议短期固定：

- `gitnexus-rust-core` 自身 Rust smoke
- Cangjie live repo 中的只读生产目标
- Cangjie index checkout 中的只读目标
- 一个 Rust workspace 真实项目，如果本机有合适样本

要求：

- 只读。
- 不 clean / build 用户项目。
- 缺失时 graceful skip。
- 输出统计要记录到 closure review。

### Tier 3：人工验收样本

用于 alpha release 前：

- 选 1 个中等 Rust 项目。
- 选 1 个中等 Cangjie 项目。
- 用输出喂给 AI 做一次实际任务：理解模块、定位影响范围或生成变更风险报告。

目标不是让 AI 完全自动改代码，而是验证“静态事实能减少盲猜”。

---

## 七、AI 消费的最小接口

第一版给 AI 用，不等于必须先做 MCP。

最小接口应该是文件/命令级：

1. `summary --format json`
   - 项目规模
   - 语言
   - package / source / symbol / edge counts
   - quality gate 总览

2. `quality --format json`
   - 哪些 gate fail
   - fail 的具体原因
   - 是否适合进入下一步分析

3. `analyze --format <stable-graph-format>`
   - 完整 graph
   - nodes / edges / diagnostics
   - confidence / reason

4. 未来可选 `report --format markdown`
   - AI-friendly 项目摘要
   - 推荐入口文件
   - 高影响节点
   - 低置信度关系
   - 明确不支持区域

短期建议：

- 先把 JSON artifact 做稳。
- 再加 Markdown report。
- MCP 最后作为读取这些 artifact 的消费层，而不是反过来绑住核心设计。

---

## 八、明确不承诺的能力

为了控制工作量，第一版 production trial 明确不承诺：

### Rust 不承诺

- 完整类型推断。
- trait solving。
- proc-macro / macro expansion。
- build.rs 执行。
- 完整 cfg / feature evaluator。
- 任意第三方 crate API symbol index。
- 100% method dispatch。

### Cangjie 不承诺

- 完整 method dispatch。
- 完整 interface / extend solving。
- macro / metaprogramming 深解析。
- 所有 SDK/LSP 能力接入。
- 跨仓全局依赖图。
- 修改 live repo。

### 产品层不承诺

- UI / WebUI。
- MCP server。
- 云端服务。
- 多语言大覆盖。
- 完整 IDE 插件。
- v1.0 兼容性承诺。

这些都可以进长期路线，但不进入 alpha production trial 的验收范围。

---

## 九、吸收同类项目思路的分层计划

| 思路来源 | 借鉴点 | 短期是否做 | 原因 |
|----------|--------|------------|------|
| CodeGraphContext / Octocode | MCP 作为 AI 查询层 | 暂缓 | 先稳定 artifact，MCP 后置 |
| Code Context Graph | 版本化机器可读图谱协议 | 短期做设计 | 关系到输出稳定，是 production trial 核心 |
| Graphify | 报告层 / 多源知识整合 | 报告层可做，多源暂缓 | Markdown report 价值高，多源会开大坑 |
| Codebase-Memory | 减少 token 和模型猜测 | 立即吸收 | 这是本项目核心价值表达 |
| KGCompass / RepoGraph | 影响分析 / 变更定位 | 中期 | 依赖 graph 稳定，适合作为下一阶段卖点 |
| GitGalaxy | risk summary / graph health 指标 | 短期部分吸收 | quality gates 已有，可转成更友好的风险摘要 |

短期只吸收三件事：

1. 稳定图谱协议。
2. AI-friendly summary / report。
3. graph health / risk summary。

其余作为长期路线，不进入本轮收尾。

---

## 十、工作量控制

建议按四个阶段控范围。

### Phase 0：路线收束文档

工作量：0.5 天。

产出：

- 本 preflight。
- docs/plans index 更新。

不改代码。

### Phase 1：Production Trial Acceptance Checklist

工作量：0.5-1 天。

产出：

- 一份 checklist 文档。
- 明确命令、字段、smoke targets、stop-line。
- 不实现新功能。

### Phase 2：最小收尾实现

工作量：2-5 天。

只允许做低风险收尾：

- README / public docs 叙事清理。
- 中性 output format alias。
- scripts 默认示例改为中性命名。
- smoke target 固化。
- report/summary 如果实现，必须小而独立。

### Phase 3：Alpha trial closure

工作量：0.5-1 天。

产出：

- 跑完整 smoke。
- 记录真实项目结果。
- 记录 known limitations。
- 决定是否标记 `v0.1-alpha` 或 `alpha production trial ready`。

不进入本轮：

- 正式改名。
- crate/binary 全量重命名。
- MCP server。
- UI/Web。
- 新语言。
- 深层 trait/type/macro 求解。

---

## 十一、到什么程度可以标 v0.1 / alpha production trial

建议满足以下条件才标：

### 必须满足

- `cargo fmt --check` pass。
- `cargo test` pass。
- `cargo test --features tree-sitter-cangjie` pass。
- `scripts/smoke.sh --quick` pass。
- 完整 smoke 在当前机器可通过或只有 documented graceful skip。
- README 公开定位清楚。
- LICENSE 存在。
- 输出格式有 alpha stable 名称。
- quality gates 文档化。
- Rust / Cangjie capability matrix 文档化。
- known limitations 文档化。

### 建议满足

- 至少 1 个真实 Rust 项目 smoke。
- 至少 1 个真实 Cangjie 项目 smoke。
- AI-friendly summary/report 初版。
- `PROVENANCE.md` 或类似说明，降低公开误解。

### 不作为阻塞

- 没有 UI。
- 没有 MCP。
- 没有 release CI。
- 没有多语言。
- Rust method dispatch 未完全解决。
- Cangjie interface/extend/method dispatch 未完全解决。

---

## 十二、推荐下一步

下一步不要直接开大功能。

建议开：

`2026-05-09-production-trial-acceptance-checklist.md`

或者如果要保持流程：

`2026-05-09-production-trial-acceptance-execution-card.md`

它只做三件事：

1. 固化 alpha production trial 的验收清单。
2. 确认 public docs / output format / smoke targets 的最小收尾 write set。
3. 明确哪些工作进入后续长期路线，不进入短期收尾。

如果本轮只想最小闭环，则下一刀推荐：

**Production Trial Acceptance Checklist（docs-only）**

而不是立刻写 MCP、UI、改名或新语言。

---

## 十三、Raw notes

输入依据：

- `docs/plans/2026-05-09-product-positioning-and-rename-preflight-draft.md`
- `docs/plans/2026-05-09-productization-phase-closure-review.md`
- `docs/plans/README.md`
- `README.md`
- 当前 Rust / Cangjie productization 状态

本文件为路线收束 preflight，不是最终发布方案。
