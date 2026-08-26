# Preflight v2：多语言项目的语言身份与文件夹体检能力

日期：2026-08-24 · 状态：**v2，已按第三方评审修订**；G2 已单独冻结为执行卡 · 作者：Kimi
评审来源：用户提供的第三方 AI 评审（评审意见的全部事实性断言已对代码核实通过：
`SOURCE_EXTENSIONS` 位置、`detect_by_extensions` 的 ≥2 阈值与单语言上报、
扫描器 5 处共享消费点、autoEntry 已有安全段）。

> v1 → v2 主要变化：拆成三张执行卡（G2 先行）；§3.2 体检契约按评审六点重写；
> §4.1 节点 language 按评审表格冻结；§6 影响面结论修正；§9 五个开放问题全部有结论。

## 1. 问题陈述

真实项目是多语言混合的。现状三个断点：

1. CLI analyze 是单语言原子操作；`--language auto` 遇多项目根目录只返回
   `workspaceAutoEntry.v1` 清单，不回答"这个文件夹里到底有什么"。
2. webui.snapshot.v1 节点没有语言身份，图谱和文件夹树无法表达语言。
3. 不支持的文件（Java 等）被静默跳过，用户无法区分"没有代码"和"不认识"。

## 2. 目标 / 非目标

目标：G1 文件夹体检；G2 语言身份进快照；G3 界面语言流程打通。

非目标：
- N1：不发明跨语言调用边（FFI/WASM/IPC），跨语言调用在 limitations 显式声明未解析。
- N2：不改 MCP server 任何现有输出 schema，**也不改共享扫描器的现有行为语义**（见 §6）。
- N3：不改 `analyze --language auto` 现有 workspaceAutoEntry 行为与输出。
- N4：本阶段不做合并多语言快照（后续独立卡）。
- N5：不支持 Java 分析本身（只做到"看见并标记"）。

## 3. 设计：文件夹体检（G1，卡 2 —— 契约按本节冻结后才开工）

### 3.1 分层检测 + 置信度

| 层 | 证据 | 置信度 |
|---|---|---|
| L1 manifest 背书 | Cargo.toml / package.json / pyproject.toml / pom.xml 等 | `certain` |
| L2 配置文件 | tsconfig.json / .clangd / CMakeLists.txt 等 | `high` |
| L3 扩展名直方图 | 源码文件统计 | `medium` |
| L4 不支持 | 命中已知但不支持的扩展名 | `medium` |

阈值（评审结论 Q2）：**生产者不按"够格"过滤**，每行都带 `sourceFileCount`，
"够不够格展示"由 UI 滤。若生产者必须滤：unsupported **≥1** 就报（1 个 .java 也要看见），
L3 可分析源码区**对齐现有扫描器的 ≥2**，不新造阈值。

### 3.2 输出契约（v2，评审六点已并入）

```
codelattice inspect --root <PATH> [--format json]
→ schemaVersion: "codelattice.workspaceInspection.v1"
```

冻结规则：

1. **一行 = `(relativePath, language)`**。同一目录多种语言就多行
   （17 个 .py + 4 个 .sh → 两行）。`sourceFileCount` / `evidence` 只描述这一行。
   禁止"一个目录绑定一种语言"的形状（事后只能破坏性改）。
2. **evidence 是结构化对象**，不是自由字符串：
   `{"kind":"manifest","file":"Cargo.toml"}` /
   `{"kind":"config","file":"tsconfig.json"}` /
   `{"kind":"extension-histogram","extension":".java","count":12}`。
3. **不是 autoEntry 的超集，是新 schema**：共享底层扫描，两套信封
   （`projects` / `sourceOnlyAreas` / `unsupportedAreas` vs autoEntry 的
   `supportedProjects` / `sourceOnlyAreas` / `unsupportedModules`）。文档与注释必须
   写明这一点，防止有人把 autoEntry 往 inspection 上靠而破 N3。
4. **必带安全段**：`generatedFrom: {staticAnalysis: true, projectContentRead: false,
   scriptsExecuted: false}` + `cautions[]`。没有这两段，体检卡看起来像"我们扫过源码"。
5. **`analyzable` = 这台二进制现在能跑**：语言在支持列表但当前 dev 二进制没编该
   feature 时，`analyzable: false` + `reason: "language-support-disabled-in-this-binary"`；
   语言不支持时 `reason: "language-not-supported"`。
6. **嵌套规则**：manifest 项目罩住的子目录不再单独成 area（frontend 是 TS 项目时
   不再把 frontend/src 报成 sourceOnly）。
7. **unsupported 行加 `recognition`**：`"known-unsupported"`（.java/.go/.rb：认得出、
   不会做）vs `"unrecognized"`（自定义扩展名：可能是生成物）。现在加是加法，以后加是迁移。

```jsonc
{
  "schemaVersion": "codelattice.workspaceInspection.v1",
  "root": "...",
  "generatedAt": "...",
  "generatedFrom": {"staticAnalysis": true, "projectContentRead": false, "scriptsExecuted": false},
  "projects": [
    {"name": "backend", "relativePath": "backend", "language": "rust",
     "confidence": "certain", "evidence": {"kind": "manifest", "file": "Cargo.toml"},
     "sourceFileCount": 42, "analyzable": true}
  ],
  "sourceOnlyAreas": [
    {"relativePath": "scripts/tools", "language": "python", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".py", "count": 17},
     "sourceFileCount": 17, "analyzable": true},
    {"relativePath": "scripts/tools", "language": "shell", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".sh", "count": 4},
     "sourceFileCount": 4, "analyzable": true}
  ],
  "unsupportedAreas": [
    {"relativePath": "legacy", "language": "java", "confidence": "medium",
     "evidence": {"kind": "extension-histogram", "extension": ".java", "count": 12},
     "sourceFileCount": 12, "analyzable": false,
     "reason": "language-not-supported", "recognition": "known-unsupported"}
  ],
  "cautions": ["..."],
  "recommendedNextActions": ["..."]
}
```

### 3.3 实现路径：与现有扫描器的关系

体检是**更富的序列化 + 新增少数检测路径**，不是从零发明：

- 复用 `scan_workspace_inventory` 的遍历 / manifest 表 / 支持分类。
- L3 多语言逐行上报、L4 `recognition` 分档需要**新函数**，不得改
  `detect_by_extensions` 的"≥2 才报 + 只报最多语言"旧行为（它被 autoEntry / MCP 共用）。
- 扫描器共享改动只允许：加内部字段、加新函数、加新 flag。autoEntry 的序列化测试
  必须继续咬住现有 JSON。

## 4. 设计：语言身份进快照（G2，卡 1 —— 已冻结，见单独执行卡）

冻结规则（评审确认，契约级）：

| 规则 | 结论 |
|---|---|
| 缺席语义 | 未知。**禁止 `""`，禁止 `null`**（TS 里 null ≠ 缺席）；只有缺席 |
| package 节点 | 不带 `language`（容器跨语言） |
| 符号节点 | 跟所属文件走；`file` 为空就省略（如 shell 的 command/env 符号） |
| 文件节点 | 优先 `properties.language`，否则扩展名映射表 |
| moduleGraph 模块 | `languages: string[]`，**字母序去重**；全未知则省略字段，禁止 `[]` |

扩展名表**复用** `workspace-model` 的 `SOURCE_EXTENSIONS`（需导出为 pub，加法改动），
不在 `webui_snapshot.rs` 另写一份。该表含 csharp/go/java/kotlin/swift 等不可分析语言：
用于节点身份标注合法（身份 ≠ 可分析性）。已知误伤：`.h→c` 在 C++ 项目里会标错，
**写进 limitations，不在转换器里猜**。

**禁止**把 `language` 写进 0.3.0 的 analyze 图节点——那才会碰 MCP 的读取面。

前端消费：结构树文件/文件夹徽标 + 图谱节点角标；文件夹徽标 = 子孙语言去重
（纯展示聚合，评审边界：检查器 / Chat 不得把"这个文件夹是 Rust"当事实陈述，
那是推断；真要当事实需预聚合进快照）。

## 5. 设计：界面流程（G3，卡 3 —— 依赖卡 2）

```
用户选文件夹 → codelattice inspect（体检卡）
  → 恰好 1 个可分析项目 → 直接 analyze --format webui-snapshot
  → 多个可分析项目      → 用户挑选（语言徽标 + 置信度 + 文件数）
  → 0 个可分析项目      → 明确说明（全部 unsupported / 空目录），不静默失败
```

届时替换 App.tsx 两处写死的 `"rust"`（497 行 analyze 调用、543 行 snapshotMeta）。

## 6. 影响面分析（v2 修正）

**schema 层面零影响成立**：MCP 走自有信封、不经 `print_analyze_result`、不读
webui.snapshot.v1；新动词 `inspect` 现有工具不会自动调用；节点 `language` 是
webui 快照可选字段，MCP 图查询碰不到。

**行为层面零影响不成立**（v1 文档的错误，已修正）：`scan_workspace_inventory` 被
CLI autoEntry（lib.rs:710）、CLI 前缀匹配（lib.rs:1218）、MCP server
（mcp_server.rs:745 / 786 / 23343）、MCP job（mcp_job.rs:1193）共 5 处共享。
若"升级"检测语义（什么算项目 / 阈值 / 是否收录 java 区），autoEntry 的
`supportedProjectCount` / `sourceOnlyAreas` / `unsupportedModuleCount` 都会变——
schema 没变，agent 看到的清单变了。因此 §3.3 的隔离规则是强制项。
"第一步全量 cargo test 已绿"只证明尚未改扫描器，不证明升级后零影响。

## 7. 后续路线（各自独立卡）

1. 合并多语言快照（体检驱动编排；子图并集；跨语言边 = N1 不做）。
2. Python snapshot-gen 脚本退役（core 转换器补齐 redact / explore / quality 段后）。
3. MCP 复用 core 转换器；agent 侧体检入口（`codelattice_workspace mode=inspect`，另一张卡）。

## 8. 测试策略

- 卡 1：转换器单测（已知答案）+ CLI 集成测试补 language 断言；前端 vitest 徽标渲染；
  不重生成全部 fixture（单语言项目徽标"全是 rust"是预期）。
- 卡 2：fixtures 构造四混合目录（manifest / config / 源码区 / java 区）契约测试锁定
  四层置信度 + 一行一语言 + 嵌套规则 + analyzable 对当前二进制；autoEntry 序列化
  回归测试必须继续绿。
- 全程 TDD；stats 实算；不产 dangling 边。

## 9. 开放问题结论（评审已拍）

- Q1 独立动词 ✓（体检不是分析；agent 入口是另一张卡）。
- Q2 生产者不设"≥3"阈值 ✓（详见 §3.1）。
- Q3 省略、禁 null ✓（卡 1 冻结项）。
- Q4 目录徽标放前端 ✓（附事实/推断边界，见 §4）。
- Q5 unsupported 分 `recognition` 两档 ✓（见 §3.2 规则 7）。

## 10. Stop-line 合规自查

- 不碰 calls.rs 质量看护区 ✓
- 不做类型推断 / trait 求解 / 宏展开 / cfg 求值 ✓
- 体检不执行项目代码、不读源码内容 ✓（继承扫描边界，`projectContentRead: false`）
- 不产 dangling 边、不发明跨语言边 ✓
- stats 实算不硬编码 ✓
- 不改 GitNexus-RC、不改活仓库、不提交 git ✓
