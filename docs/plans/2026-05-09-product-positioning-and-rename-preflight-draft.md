# Product Positioning and Rename Preflight Draft

> **日期：** 2026-05-09
> **状态：** Draft / 初稿，非最终方案
> **目的：** 帮助判断当前项目所处位置、完成度、可改动性、工程量，以及是否需要从“GitNexus Rust Core”切换到独立产品定位。
> **Stop-line：** 本文只做定位研究和工程量评估，不改 runtime、不改 CLI、不改 schema、不做最终命名决策。

---

## 一、先给结论

当前项目已经不是“玩具原型”，而是一个**本地可运行、可回归、可继续产品化的代码上下文核心**。

但是它也还不是成熟开源产品，主要原因不是代码完全不能用，而是：

1. **公开定位还不干净**：README 已经初步中文化，但仓库内仍有不少 `GitNexus-RC`、`bridge`、`migration` 叙事。
2. **名字仍绑定旧路线**：`GitNexus Rust Core` 会让外界误以为这是 GitNexus 的官方 Rust 版、fork、移植版或兼容层。
3. **消费协议还没独立命名**：`--format gitnexus-rc` 已可用，但作为公开协议名不合适。
4. **工程能力已经进入深水区**：Rust 和 Cangjie 都有真实图输出、质量门和 fixtures，后面主要是语义深度、协议稳定、产品入口。

更准确的阶段判断：

| 维度 | 当前阶段 | 粗略完成度 | 判断 |
|------|----------|------------|------|
| 分析核心 | 可用原型 -> 本地试用候选 | 65%-75% | Rust/Cangjie 双线都有结构化输出和质量门 |
| 开源产品形态 | 早期公开仓库 | 35%-45% | README、许可证已补，但名字/公开叙事/API 命名还未清理 |
| Rust 语言深度 | 中等静态分析 | 55%-65% | 项目模型强，调用解析可用但 method/trait/type 仍是主要 gap |
| Cangjie 语言深度 | 早期深度支持 | 60%-70% | 仓颉支持是差异点，已覆盖 cjpm/symbol/import/reference/call/diagnostics |
| 分发可用性 | 本地构建可用 | 30%-40% | 有 build/smoke，缺 release 包、安装方式、版本策略 |
| AI 消费层 | 基础数据具备 | 25%-35% | 有 graph JSON，但 MCP/IDE/报告层还没形成产品入口 |

一句话：**技术底座已经成形，产品身份还没成形。**

---

## 二、同类方向初步观察

这条路线不是 GitNexus 独有，外部已经有多个相近方向。下面不是完整竞品审计，只是帮助我们判断自己站在哪。

| 项目/方向 | 主要特点 | 对我们的启发 |
|-----------|----------|--------------|
| CodeGraphContext | 面向 AI assistant 的本地代码图谱和 MCP 查询 | MCP 可以作为消费层，但核心不应被 MCP 绑定 |
| Octocode | Rust + Tree-sitter + MCP，偏代码搜索和语义上下文 | Rust-native + Tree-sitter 是合理路线 |
| Code Context Graph | 强调机器可读、可发布的代码上下文图格式 | 我们需要独立命名的输出协议，而不是 `gitnexus-rc` |
| Graphify | 把代码、文档、图片等转成知识图谱 | 可借鉴“报告层”和多源知识，但不宜一开始做太宽 |
| Codebase-Memory | Tree-sitter + MCP，目标是减少 token 和工具调用 | 证明“静态事实先算出来”是对 AI agent 有价值的 |
| KGCompass / RepoGraph 类论文 | 用代码图帮助 repo-level repair / 定位影响范围 | 后续可以把“影响分析”作为核心卖点，而不只是画图 |
| GitGalaxy 类可视化工具 | 强调低 token、风险点、架构可视化 | 可借鉴风险摘要和 graph health 指标表达 |

初步判断：

- 不要把项目定位成“另一个代码搜索工具”。
- 不要主打“支持很多语言”，这条赛道竞争很拥挤。
- 不要让模型猜测项目结构；应该让静态分析先产出可验证事实。
- 我们应该主打：**本地、确定性、Rust-native、语言深度、Cangjie/Rust 双重点、质量门、confidence/reason 可解释图谱**。

---

## 三、建议的新定位方向

当前推荐的产品定位初稿：

> 一个本地优先、Rust 编写、面向 AI 编程助手和工程质量检查的确定性代码上下文引擎。它把 Rust 与 Cangjie / 仓颉项目转换成可验证的项目模型、符号表、引用关系、调用关系、诊断信息和代码图谱，让 AI 少猜、少读全仓、少浪费 token。

这句话里有几个关键点：

1. **本地优先**：代码不需要发到云端。
2. **确定性**：不是 LLM 猜结构，而是工具先算事实。
3. **Rust 编写**：单二进制、性能、可分发性是优势。
4. **AI 友好但不依赖 AI**：AI 是消费方，不是事实生成方。
5. **Cangjie / Rust 深度优先**：不追求一开始覆盖几十种语言。
6. **可验证图谱**：每条关系要有 reason/confidence，质量门可回归。

---

## 四、当前项目真正完成了什么

### 4.1 已经扎实的部分

- 有 Rust workspace 和多 crate 结构。
- 有统一 CLI：`analyze` / `quality` / `summary`。
- 有 Rust 项目模型、符号提取、import/call/graph 输出。
- 有 Cangjie 项目模型、cjpm、tree-sitter、symbol/import/reference/call/diagnostics/graph 输出。
- 有 quality gates：duplicate、dangling、deterministic、synthetic 等。
- 有 fixture corpus 和 graph contract tests。
- 有 build/smoke 脚本。
- 有 MIT LICENSE。
- README 已开始转向公开中文介绍。

这说明项目的核心不是“空想”，而是已经可以作为本地工具继续深挖。

### 4.2 还不成熟的部分

- 名字仍叫 `GitNexus Rust Core`，公开认知风险较高。
- README 仍有 `Bridge 格式验证（面向 GitNexus-RC adapter 开发者）`。
- docs 中还有大量“迁移 / GitNexus-RC / bridge / adapter / upstream”旧路线语言。
- CLI 输出格式仍有 `--format gitnexus-rc`，需要改成独立协议名或保留兼容别名。
- 没有 MCP server / IDE 插件 / HTML report / 项目报告层。
- 没有 release packaging / install docs / versioned schema policy。
- 没有独立品牌叙事：用户不知道它和同类工具相比强在哪。

### 4.3 技术深水区

Rust 线：

- 已经能做中等深度静态分析。
- 项目模型和 graph contract 较稳。
- 主要硬仗是 method dispatch、trait solving、macro/cfg、external crate API。
- 这些不适合一次性手搓大一统，需要继续保持 conservative/no-edge/low-confidence 策略。

Cangjie 线：

- 差异化最强。
- 目前已经不是“只识别 AST”，而是有 cjpm/project/symbol/import/reference/call/diagnostics/graph。
- 下一步更有价值的是接 SDK/LSP/cjc/cjlint 能力，校验或增强手写分析。
- method dispatch、interface/extend、macro/meta-programming 仍需分层推进。

---

## 五、可改动性评估

### 5.1 低成本可改

这些可以很快做，风险低：

- README 改名和定位文本。
- GitCode 仓库介绍。
- docs index 中新增“产品定位 / 命名初稿”。
- 把 public-facing 的 `GitNexus-RC adapter` 表述改成“consumer format / downstream adapter”。
- 新增 `PROVENANCE.md` 或 `ACKNOWLEDGEMENTS.md`，说明项目是 Rust 独立实现，受通用代码智能/代码图谱方向启发。
- 把“bridge”在公开文档中改成“consumer format / graph export format”。

预估：0.5-1.5 天。

### 5.2 中等成本可改

这些会动脚本、测试、文档，但不一定破坏核心：

- 给 `--format gitnexus-rc` 增加中性别名，例如 `--format context-graph-v0` 或 `--format graph-v0`。
- README / scripts / docs 全面改用新格式名，旧格式名保留为 compatibility alias。
- 将 `scripts/verify-bridge.sh` 改名为 `scripts/verify-consumer-format.sh`，保留旧脚本 wrapper。
- 将 `bridge_roundtrip` 测试命名逐步改为 `consumer_format_roundtrip`。
- 把 `docs/migration/` 移到 `docs/internal/legacy/` 或重写为“历史研究记录”，避免公开入口直连。

预估：1-3 天。

### 5.3 高成本或高风险

这些应该单独开卡，不能顺手做：

- 仓库改名、crate 改名、binary 改名。
- 输出 schema v1 定稿。
- 移除所有 `GitNexus-RC` 字符串，而不是只清理 public-facing 文档。
- 删除/重写 migration docs。
- 重命名大量 Rust module / test / CLI flag。
- 加 MCP server / HTTP server / IDE 插件。
- 做完整 type inference / trait solving / macro expansion。

预估：

| 工作 | 粗略工程量 |
|------|------------|
| 仓库公开叙事清理 | 0.5-1.5 天 |
| 中性 format alias + docs/scripts/tests 对齐 | 1-3 天 |
| 项目正式改名（repo/binary/crate/docs） | 2-5 天 |
| 独立 schema v1 和兼容策略 | 3-7 天 |
| MCP query layer MVP | 3-7 天 |
| HTML / Markdown 项目报告层 | 2-5 天 |
| Rust method/trait 深水区 | 2-6 周，需分很多小 slice |
| Cangjie SDK/LSP 深水区 | 1-4 周，取决于 LSP 能力和稳定性 |

---

## 六、推荐路线初稿

### Phase A：公开身份清理

目标：让外界第一眼知道“这是一个独立开源代码上下文引擎”，不是某个内部 fork。

建议：

1. 起一个临时 codename，但不急着最终定名。
2. README 第一屏去掉 `GitNexus Rust Core` 作为产品名，改为临时占位名。
3. README 不再提 `GitNexus-RC adapter`。
4. 把“bridge”公开叙事改成“consumer graph format”。
5. 新增 `PROVENANCE.md`：说明独立 Rust 实现、开源研究、未复制上游实现代码、第三方依赖遵循各自许可证。

### Phase B：协议独立化

目标：输出格式不再叫 `gitnexus-rc`。

建议：

1. 新增中性 `--format context-graph-v0`。
2. 保留 `--format gitnexus-rc` 作为 deprecated alias，只在内部 docs 中出现。
3. 将 README、build.sh、smoke.sh、verify 脚本默认示例切到新格式。
4. 输出 JSON 顶层加入 `schemaName` / `schemaVersion`，为未来消费层做准备。

### Phase C：产品入口增强

目标：让普通用户不需要懂 graph，也能看到价值。

建议：

1. 增加 `report` 命令或 `summary --format markdown`。
2. 输出项目结构摘要、核心模块、调用热点、低置信度关系、质量门结果。
3. 给 AI agent 一个“推荐上下文入口”：哪些文件先看、哪些函数是高影响点。

### Phase D：语言深度继续推进

目标：保持差异化，而不是盲目铺语言数量。

建议优先级：

1. Cangjie SDK/LSP diagnostics/documentSymbol/references smoke。
2. Cangjie interface/extend/method ownership 深水区。
3. Rust method call 分类和 reason/confidence 矩阵。
4. Rust cargo metadata 可选模式，而不是默认执行。
5. Macro/cfg/trait solving 保持 stop-line，除非单独开长期设计。

---

## 七、命名方向初稿

不是最终命名，只先列命名约束。

应该避免：

- `GitNexus`
- `GitNexus Rust Core`
- `gitnexus-rc`
- 任何暗示“官方 fork / 迁移版 / 兼容层”的名字
- 太泛的 `CodeGraph` / `RepoGraph`，容易撞名或难搜索

名字最好表达：

- 本地代码上下文
- 静态事实图谱
- AI agent 可消费
- Rust-native / deterministic
- 对 Cangjie 友好，但不把名字限定死为仓颉工具

可探索方向：

| 方向 | 示例类型 | 备注 |
|------|----------|------|
| Graph / Atlas | `CodeAtlas` 类 | 易懂，但撞名概率高 |
| Context / Memory | `RepoContext` 类 | AI 友好，但容易泛 |
| Deterministic / Signal | `CodeSignal` 类 | 能表达少猜测，但需查重 |
| Chinese-inspired | `Jingwei` / `Luoshu` 类 | 有辨识度，但国际用户理解成本高 |
| Engine/Core | `ContextCore` 类 | 稳，但不够有品牌感 |

下一步如果要定名，应单独做：

1. GitHub/GitCode/crates.io/npm/domain 查重。
2. 商标粗查。
3. README 首屏可读性测试。
4. CLI binary 可输入性测试。

---

## 八、最值得借鉴的产品思路

不是照抄任何项目，而是吸收共性好设计：

1. **稳定图谱协议**
   输出不只是“当前 CLI JSON”，而是一个版本化 consumer contract。

2. **MCP 作为消费层**
   MCP 很适合 AI，但不要让 MCP 侵入核心。核心仍是 Rust analyzer + artifact。

3. **报告层降低门槛**
   普通用户不一定看 graph JSON。需要 Markdown/HTML 摘要告诉他“项目是什么、风险在哪、AI 应该从哪看”。

4. **低置信度诚实标注**
   不确定就 `low-confidence` 或 no-edge，不假装完整语义。

5. **影响分析优先于大而全问答**
   “改这里会影响什么”比“这个项目介绍一下”更有工程价值。

6. **语言深度优先于语言数量**
   Rust + Cangjie 做深，比浅浅支持 20 种语言更有差异。

---

## 九、当前风险

| 风险 | 说明 | 建议 |
|------|------|------|
| 公开叙事混乱 | README 已改善，但 docs/scripts 仍有旧路线语言 | 先清 public-facing，再处理 internal legacy |
| 名字绑定 GitNexus | 用户可能误解为官方关联/fork | 尽快定临时新名 |
| 格式名绑定 `gitnexus-rc` | CLI 示例和脚本仍强化旧关系 | 新增中性 format alias |
| 过早大改名 | 大量文件/测试/脚本会被牵动 | 先做 alias 和文档清理，再做正式 rename |
| 语义过度承诺 | Rust trait/macro/Cangjie method dispatch 都很难 | 保持 capability matrix 和 stop-line |
| 竞品叙事焦虑 | 同类项目多，但不等于没有空间 | 聚焦本地、确定性、语言深度、仓颉 |

---

## 十、建议的下一刀

推荐下一刀不是最终改名，而是：

**Public Identity Cleanup Execution Card**

Write set 候选：

- `README.md`
- `docs/plans/README.md`
- `docs/architecture/consumer-contract.md`
- `docs/architecture/output-contract.md`
- `scripts/build.sh`
- `scripts/smoke.sh`
- `scripts/verify-bridge.sh`（可能新增 wrapper，不一定直接改名）
- 新增 `docs/plans/2026-05-09-public-identity-cleanup-execution-card.md`
- 新增 `PROVENANCE.md`（可选）

Stop-line：

- 不改 runtime analysis。
- 不改 graph schema。
- 不删除历史 docs，只做公开入口和当前说明清理。
- 不做最终命名。
- 不移除 `--format gitnexus-rc`，最多新增中性 alias。

验收：

- README 第一屏不再出现 `GitNexus-RC` / `bridge adapter`。
- 快速开始里只出现中性输出格式。
- GitCode 仓库介绍和 README 统一。
- 历史 docs 中的旧名只作为 historical/internal 出现。
- `rg` 能区分 public-facing hits 和 historical hits。

---

## 十一、Raw notes

本轮读取/观察：

- `README.md`
- `AGENTS.md`
- `GOVERNANCE.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-09-productization-phase-closure-review.md`
- `Cargo.toml`
- `crates/`
- `fixtures/`
- `scripts/`

本轮只做文档初稿，不执行最终方案。

外部方向参考入口（需后续二次核验）：

- CodeGraphContext: https://codegraphcontext.github.io/
- Octocode: https://octomind.run/product/octocode
- Code Context Graph: https://www.codecontextgraph.com/
- Graphify: https://graphify.net/
- Codebase-Memory paper: https://arxiv.org/abs/2603.27277
- KGCompass paper: https://arxiv.org/abs/2503.21710
- GitGalaxy: https://gitgalaxy.io/
