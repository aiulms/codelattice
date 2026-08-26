# 总体规划：打通桌面工作台的真实多语言项目分析

- 日期：2026-08-24 · 状态：**v2，已按第三方复核修订** · 作者：Kimi
- v2 修订要点：P0 诊断补 main/rayon 线程区分；P1 改显式 feature 列表；
  P2 冻结信封 `languages[]` 加法、合并先于截断、relationKey 重算、失败策略、
  命令命名禁用 auto；阶段顺序改为 P0/P1 并行。
- 前置文档：`2026-08-24-polyglot-language-identity-preflight.md`（preflight v2）、
  `2026-08-24-cli-webui-snapshot-format-execution-card.md`（方案 B）、
  `2026-08-24-workspace-inspect-command-execution-card.md`（卡 2）、
  `2026-08-24-desktop-project-picker-execution-card.md`（卡 3）
- 治理约定不变：每个阶段仍先冻结执行卡再开工；本规划只定方向、边界与依赖，
  不替代执行卡。

## 1. 目标态

用户在桌面工作台选中 `open-nwe` 这类多语言仓库根目录，一次操作得到一张
**覆盖全部可分析语言的合并图谱**：节点带语言身份，结构树/模块图/图谱按语言
区分，unsupported 区域可见但明确标记。跨语言调用边**不做**（preflight N1，
保持冻结）。

## 2. 现状盘点（2026-08-24 收盘）

已交付并复核通过：

| 项 | 内容 | 证据 |
|---|---|---|
| 方案 B | CLI `--format webui-snapshot`，转换器在 `crates/cli/src/webui_snapshot.rs` | 转换器单测 + CLI 集成测试绿 |
| 卡 1 | 快照节点 `language` 字段（含 moduleGraph `languages` 聚合规则） | 契约测试绿 |
| 卡 2 | `codelattice inspect` 体检命令（workspaceInspection.v1） | workspace-model 18/18、CLI 契约 7/7、根全量绿 |
| 卡 3 | 桌面挑选器（选文件夹→体检→挑项目→出图） | src-tauri 21/21、vitest 131/131、tsc 干净；inspect-smoke 手工点验通过 |

暴露的新问题（真实仓库 open-nwe 点验时发现）：

1. **P0 阻塞**：`codelattice analyze --root open-nwe/backend --language rust`
   栈溢出（SIGABRT）。`--format json` 老路径同样崩 → 核心层前序 bug，非本批
   卡片引入。已定位方向：MCP 路径分析跑在 16MB 栈独立线程（`mcp_job.rs:760/1187/1278`），
   rayon 池 8MB（`project-model/src/item.rs:88`），而 CLI analyze 在主线程裸跑
   （macOS 主线程栈 8MB）。
2. **P1 可用性**：CLI `default = ["tree-sitter-extraction"]`，dev 二进制只有
   rust+shell；open-nwe 的 frontend（typescript，506 文件）在挑选器里灰显
   `language-support-disabled-in-this-binary`。
3. 合并多语言快照未做（preflight N4，本来就是后续卡）。

## 3. 阶段划分与依赖（v2 顺序）

```
P0 栈溢出诊断+修复   ──┐
P1 构建脚本/文档     ──┘ 可并行（P1 的「出图」验收依赖 P0 通路）
        ↓
P2 合并多语言快照（需要 P0 通路 + 至少 rust/ts/python 能跑；不得提前并行——
   合并循环套在会溢栈的 analyze 上测不出合并本身）
        ↓
P3 Python 脚本退役 ──┐ 并行；P3 必须在 P2 之后（对齐基准会变）
P4 MCP 对齐        ──┘
```

## 4. P0：analyze 栈溢出修复

**目标**：open-nwe/backend（612 个 rust 文件）analyze 全程跑通。

**先诊断，不许直接加栈**（v2 强化）：

1. **第一刀就区分线程**：`lldb -o run -o bt` 或崩溃时 `sample`，看栈顶函数
   与线程名——main 还是 rayon worker。这决定修法：main 溢出 → 包大栈线程
   有效；rayon worker（已是 8MB 池）溢出 → 包主路径线程**没用**，要调
   `item.rs` 的 rayon `stack_size` 或给那份递归加界。
2. 若很快 abort，多半是解析/模块递归，不是序列化。
3. 判定递归**深但有界**（大项目自然深度）还是**可能无界**（循环模块依赖、
   缺 visited 标记）。无界递归加栈只是推迟崩溃，必须修算法。
4. 项目子集二分作为补充手段，不是首选。
5. 诊断代码（深度计数日志等）不入库。

**修法矩阵**：
- main 有界深递归 → CLI analyze 主路径包进大栈独立线程，对齐 `mcp_job.rs`
  `std::thread::Builder::stack_size` 模式；16MB 起步，open-nwe 实测不足再升，
  **禁止拍 1GB**。
- rayon worker → 只调 `item.rs` 池的 `stack_size` 数字（带注释说明定量依据）。
- 无界递归 → 修算法；若触及 `calls.rs` 质量看护区，先按 AGENTS.md 评估
  拆分与 fixture 要求。
- **修在 CLI 进程内**（桌面 spawn 的是 `target/debug/codelattice`），
  不改 Tauri supervisor。
- 活仓库 open-nwe **只读**。

**验收**：open-nwe/backend `--format json` 与 `--format webui-snapshot` 均
exit 0 且产物 schema 正确；根全量测试绿；`scripts/codelattice-precommit-check.sh`
通过；新增回归测试（合成深层嵌套 fixture，用小栈线程在测试内复现原崩溃，
修复后通过）。

## 5. P1：全量语言构建与桌面二进制管理

**目标**：桌面工作台 spawn 的 CLI 具备全部语言 feature，inspect 全语言
`analyzable: true`。

设计要点（v2 修订）：
- **显式 feature 列表，禁止 `--all-features`**（workspace 以后新增 optional
  feature 会被默默编进来）。写死：
  `tree-sitter-extraction,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-python,tree-sitter-c,tree-sitter-cpp,tree-sitter-cangjie,tree-sitter-arkts`
  （shell 非 optional，无需列）。
- 固化进 `scripts/`（如 `codelattice-build-workbench-cli.sh`）+ 桌面 dev 文档。
- **不另造二进制 feature 探测机制**：inspect 的 analyzable 已天然反映。
- **风险**：`tree-sitter-cangjie` / `tree-sitter-arkts` 依赖 vendor 语法源码，
  编译时间与平台兼容性需实测；编不过列入「构建不过」清单并文档明示，
  **不改 vendor**。

**验收**：全 feature 构建绿；`inspect --root open-nwe` 中 typescript/python
行 `analyzable: true`；frontend 可点**且出图**（此条依赖 P0 通路，
P1 单独完成时不勾选）。

## 6. P2：合并多语言快照（核心战役，v2 大修）

**目标**：选 open-nwe 根 → 一次操作 → 一张覆盖 rust+typescript+python+…的图。

### 6.1 三个决策点（复核已拍，冻结）

- **Q1 合并在 core CLI** ✓。前端合并会把 id、路径前缀、moduleGraph 规则
  再写一遍，违背方案 B 单一事实源。桌面只加「全部分析」入口去 spawn。
- **Q2 schema 保持 v1，加法兼容** ✓：信封顶层加**可选** `languages: string[]`
  （现在是单数 `language`，合并图若仍写 `"language": "rust"` 会让标题栏和
  缓存键撒谎）；单语言快照继续只写 `language`；不升 v2。
- **Q3 CLI 单命令内存编排** ✓：对每个 analyzable 行依次 analyze、内存合并、
  一次输出；复用 supervisor / 取消 / 发布守卫。**命令命名禁止沾 `auto`**
  （现有 `--language auto` 是多项目根返回 workspaceAutoEntry.v1 的语义，N3）；
  用独立动词/开关，如 `analyze-workspace` 或 `analyze --workspace-merge`。

### 6.2 冻结规则（v2 逐条补强）

1. **合并发生在截断之前**：`webui_snapshot.rs` 现有 `MAX_NODES=150` /
   `MAX_EDGES=300`（已核实，webui_snapshot.rs:16-17）。若各子项目先截断再并，
   合并图是一堆残片。必须冻：在**未截断的 0.3.0 子图**上合并，再按
   「先 file/package/module，后符号」统一截断一次；或模块图不截断、符号图
   按需。open-nwe（612+506 文件）不处理这个，「一张覆盖全部语言的图」会假绿。
2. **id 命名空间**：文件节点今天是 `file:src/lib.rs`，两个子项目必撞。
   每个子图节点 id 加项目前缀；**`file` 路径改成仓库相对路径**
   （`backend/src/lib.rs`）——这比只改 id 更重要，结构树靠它并目录。
3. **relationKey 重算**：`relationKey = sha256(source\0kind\0target)`
   （已核实，webui_snapshot.rs:217-226）。id 改后必须按最终 id 重算，
   **禁止沿用子图旧 key**；契约测试更新已知答案。
4. **失败策略**：某一语言失败 → 其余语言照常合并，在 `limitations` 里
   逐条点名失败的语言与原因；**不整单失败**（整单失败会让一个坏掉的小
   区域毁掉整仓分析）。全部失败才整单 Failed。
5. **取消与进度**：编排循环每段语言都要检查 cancel（不能只包最外层大栈
   线程）；全量可能数分钟，桌面轮询的 `Running` 之外最好有
   「正在分析 typescript (2/3)」级状态，否则用户以为卡死。
6. **unsupported 进元信息不进图**（N5 保持）：合并快照加结构化可选段
   `inspectionSummary`（从 inspect 信封裁剪三桶摘要），前端沿用卡 3
   挑选器的折叠形状展示「还有 N 块没进来」；**不只**靠 `limitations.notes`
   一句中文。
7. **桌面行为保留**：卡 3 的单项目直通/挑选器不动，「全部分析」是额外
   入口；合并后仍是一条 `job-*` 快照，现有 pin/query store 键空间够用，
   **不为多项目开新键空间、不按子语言各 pin 一张**。
8. moduleGraph `languages: string[]` 聚合规则沿用卡 1 冻结（字母序去重）。
9. `generatedFrom` 安全段与 `limitations` 声明跨语言调用未解析（N1 保持）。
10. 性能预算：执行卡带 open-nwe 全量的耗时与内存实测；前端默认落模块图
    视图，符号图按需展开。

**验收**：open-nwe 根一键出合并图；节点语言徽标正确；结构树按仓库相对
路径并目录；单语言失败场景有 fixture 验证部分合并 + limitations 点名；
全量测试绿；契约测试锁定 id 命名空间、relationKey 重算、languages 聚合。

## 7. P3：Python snapshot-gen 脚本退役

- 前提：core 转换器补齐 `scripts/codelattice-snapshot-gen.py` 仍在用的段落
  （redact / explore / quality——退役前逐字段 diff 对齐）。
- 退役 = 删脚本 + 改文档引用 + 确认无 CI/外部调用方。
- 排在 P2 后（P2 改快照形态，P3 的对齐基准以新形态为准）。

## 8. P4：MCP 复用转换器 + agent 体检入口

- MCP 侧复用 `webui_snapshot.rs` 转换器，消灭 MCP 与桌面的输出漂移；
  **加法**：不让 MCP 默认改吐 webui-snapshot。
- agent 体检入口：`codelattice_workspace` 加 `mode=inspect`（或等价），让
  其他 agent 能 programmatically 拿到 workspaceInspection.v1。
- 不改 MCP 现有输出 schema（preflight N2 保持）。
- 与 P3 并行，排在 P2 后。

## 9. 全局不变量（所有阶段共同遵守）

- 扫描器隔离：`scan_workspace_inventory` / `detect_project_at` /
  `detect_by_extensions` 行为语义零改动；检测能力只加不改。
- stats 实算，禁止硬编码默认值。
- 不产 dangling 边；不发明跨语言边。
- 不碰 calls.rs 质量看护区（P0 未证实前也不动；新增 CALLS 策略前先评估拆分）。
- 不改 GitNexus-RC、不改活仓库源码（open-nwe 只读）、不做 git commit。
- 每阶段一张执行卡，冻结后开工；验收数字复核人重跑。

## 10. 复核结论存档（2026-08-24）

§10 原六问已由第三方复核回答并全部并入上文（v2）。关键裁定：
Q1/Q3 倾向成立；Q2 保持 v1 但加可选 `languages[]`；P0 诊断必须区分
main vs rayon；P1 显式 feature 列表；合并先于截断；relationKey 重算；
新命令名禁沾 `auto`。下一步：冻结 P0 执行卡。
