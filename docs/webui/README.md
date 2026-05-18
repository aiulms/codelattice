# CodeLattice WebUI

> **日期：** 2026-05-18
> **状态：** WebUI Workbench + G6 Graph Engine + Cross-Project Impact 已落地
> **定位：** 本地 Runner 工作台 + enriched snapshot pipeline + AntV G6 图谱可视化 + 跨项目影响分析 + 多语言 fixture matrix

---

## 一、定位

CodeLattice WebUI 不是 MCP 的替代品，而是 MCP / CLI 结果的**人类可视化层**。

- WebUI 消费的是聚合后的只读 snapshot；Runner 模式也可以发起受控 MCP job
- MCP 仍是 AI 主通道；WebUI 是给人类看的本地项目理解界面
- 图谱视图默认使用 vendored AntV G6 渲染，保留 SVG fallback

### 核心原则

1. **Read-only** — WebUI 只展示静态分析结果，不修改项目代码
2. **Snapshot-driven** — 所有数据来自 `CodeLatticeWebSnapshotV1` JSON
3. **No runtime proof** — 不声称运行时证明、外部使用证明、覆盖证明、安全删除证明
4. **Local-first** — 本地开发工具，不上传数据，不依赖云端
5. **Heuristic transparency** — 明确标注启发式/置信度/风险理由

### 硬边界

| 边界 | 规则 |
|------|------|
| 只改 CodeLattice repo | 不碰 GitNexus-RC / Tool / CodeLattice-Tool |
| 不新增前端框架 | 无 React/Vue/Svelte/Tauri/Electron |
| 不引入包管理 | 无 npm/pnpm/yarn；G6 以静态 vendor bundle 方式随 WebUI 提供 |
| 不改 MCP 字段语义 | 只 additive 新增 |
| 不运行 promote | 不部署到 CodeLattice-Tool |

---

## 二、MVP 视图规划

WebUI MVP 包含 5 个核心视图：

| # | 视图 | 目标用户问题 | 数据来源工具 |
|---|------|-------------|-------------|
| 1 | Dashboard | 项目整体健康如何？ | `project_overview`, `quality`, `project_insights` |
| 2 | Explore | 这个符号/文件长什么样？谁调用它？ | `symbol_context`, `symbol_search`, `calls_from/to`, `query_graph` |
| 3 | Graph | 项目关系图长什么样？能否点击下探？ | snapshot `graph` section + AntV G6 |
| 4 | Cleanup | 哪些代码可能是死代码？哪些不可达？ | `dead_code_candidates`, `reachability_map`, `external_api_surface`, `framework_entry_hints` |
| 5 | Release Review | 发布前有什么风险？ | `breaking_change_review`, `consistency_review`, `config_examples_review`, `automation_graph` |

### Graph Engine

Graph 视图默认使用 **AntV G6 5.1.1**：

- G6 高级图谱：canvas 渲染、拖拽、缩放、节点点击、双击下探
- SVG 兼容图谱：G6 不可用时的 fallback
- 布局模板：代码星云、模块星团、调用流向、蓝图架构、工程探索
- 海报模式：隐藏辅助面板，放大图谱画布，适合截图传播

CodeLattice 自己负责代码语义和布局策略；G6 只作为渲染与交互引擎。

### Automation Graph Integration

Runner 模式生成 snapshot 时会 best-effort 调用 `codelattice_automation_graph`，将 `automationGraph` 写入 snapshot。MCP 不可用时不会阻断普通分析，而是标记 `status=not_collected`。

### Workspace Cross-Project Graph

工作区模式新增跨项目关系图能力：静态识别 workspace 内子项目之间的依赖、引用和边界关系。

**API：** `GET/POST /api/workspace/graph?runId=...` — 返回 `CodeLatticeWorkspaceGraphV1`

**Node Kinds：** workspace, project, package, config, script, workflow, unsupported

**Edge Kinds：** contains, depends_on, imports, script_refs, config_refs, adjacent_to, unsupported_boundary

**识别能力：**
- Cargo.toml workspace members / path dependencies
- package.json workspaces / file: dependencies
- tsconfig.json paths aliases
- pyproject.toml / cjpm.toml path dependencies
- CMakeLists.txt add_subdirectory
- Makefile / shell 脚本引用
- CI workflow (.github/workflows) 本地路径引用
- Dockerfile COPY 本地路径
- 同父目录下的 supported ↔ unsupported 模块边界

**Confidence 策略：** 1.0 (contains/direct manifest) → 0.85 (path dep) → 0.75 (config/script ref) → 0.65 (source import) → 0.45 (name-only) → 0.35 (adjacency)

**前端：** Insights 区块内新增 Cross-Project Graph Summary，包含节点/边计数、高连接度项目、桥接脚本/配置、不支持边界。可加载完整 graph 并查看 edge table。可复制图谱摘要给 AI。

**硬边界：**
- 不执行目标项目代码
- 不证明运行时依赖
- 不做完整构建系统解析
- 所有输出标注 static-only / heuristic

- Release Review 会展示自动化工作流数量和风险候选。
- Workflow 页会显示自动化图谱审查面板，辅助检查 CI、package scripts、Makefile、Dockerfile 和 shell 脚本。
- Live MCP 面板提供 `automation_graph` 工作流，可对当前项目即时运行自动化图谱审查。
- Report Export 会生成 `Automation Graph Review` 小节。

该能力仍是 **static-only**：不执行 CI、脚本、构建或 Docker 命令。

### Workspace Cross-Project Graph

工作区模式新增跨项目关系图能力：静态识别 workspace 内子项目之间的依赖、引用和边界关系。

- **API**: `GET/POST /api/workspace/graph?runId=...` — 返回 `CodeLatticeWorkspaceGraphV1`
- **Node Kinds**: workspace, project, package, config, script, workflow, unsupported
- **Edge Kinds**: contains, depends_on, imports, script_refs, config_refs, adjacent_to, unsupported_boundary
- **Confidence**: 1.0 (contains/direct manifest) → 0.85 (path dep) → 0.75 (config/script ref) → 0.65 (source import) → 0.35 (adjacency)
- **前端**: Insights 区块内新增 Cross-Project Graph Summary（节点/边计数、高连接度项目、桥接脚本/配置、不支持边界、edge table、复制图谱摘要给 AI）
- **硬边界**: 不执行目标项目代码、不证明运行时依赖、不做完整构建系统解析、所有输出标注 static-only/heuristic

### Workspace Cross-Project Graph

Workspace 模式新增跨项目关系图能力，从"能发现多个子项目"推进到"能理解多个子项目之间的关系"。

**API：** `GET/POST /api/workspace/graph?runId=...`

**输出结构：** `CodeLatticeWorkspaceGraphV1`

**Node Kinds：** workspace / project / config / script / workflow / unsupported

**Edge Kinds：** contains / depends_on / imports / script_refs / config_refs / adjacent_to / unsupported_boundary

**静态关系提取策略：**
- 只读取 manifest/config 文件（Cargo.toml, package.json, tsconfig.json, pyproject.toml, cjpm.toml, CMakeLists.txt, Makefile, CI YAML, Dockerfile, shell 脚本）
- 不执行目标项目代码
- Edge 携带 confidence (0.35-1.0)、reason 和 evidence
- depends_on：从 manifest path dependency / workspace member 提取
- script_refs：从 package.json scripts / Makefile target / shell source 提取
- config_refs：从 CI workflow / Dockerfile COPY 提取
- adjacent_to：同父目录下的兄弟模块
- unsupported_boundary：supported ↔ unsupported 边界

**WebUI 最小展示：**
- Insights 区块新增 Cross-Project Graph Summary
- 数字卡片：nodeCount / edgeCount / crossProjectEdgeCount / unsupportedBoundaryCount
- Top connected projects / Bridge scripts / Bridge configs
- "Load Full Graph" 按钮展示 edge table
- "Copy Graph Summary for AI" 按钮

**硬边界：** 不执行项目代码、不证明运行时依赖、不做图谱视觉引擎。

### Workspace Cross-Project Impact

工作区模式新增跨项目影响分析：回答"如果我改了 X，还有哪些项目会受影响？"

**API：** `GET/POST /api/workspace/impact` — 接受 target（nodeId/projectId/snapshotId/path/label/fuzzy）和 direction（upstream/downstream/both）

**输出结构：** `CodeLatticeCrossProjectImpactV1`

**Target Resolution（7 级优先级）：** exact nodeId (1.0) → projectId (1.0) → snapshotId (0.95) → exact path (0.90) → suffix path (0.75) → label (0.65) → fuzzy (0.45)

**BFS 遍历策略：**
- 在 workspace graph 上做 BFS，方向由 direction 参数控制
- `contains` 边不传播（仅 workspace→project 扩展用）
- `adjacent_to` 和 `unsupported_boundary` 视为弱边（置信度上限 0.4），不继续传播
- 路径置信度 = min(路径上所有边的置信度)

**风险评估：** critical (≥10 受影响项目) / high (≥4) / medium (≥2) / low (1) / unknown (target 未解析)

**Insights 集成：** `crossProjectImpactHints` 提供高扇出项目、共享脚本/配置、推荐影响分析目标

**WebUI 展示：** Impact 区块包含摘要卡片、受影响项目/资产表、不支持边界警告、风险原因、审查清单、"Copy AI Summary" 按钮

**硬边界：** 所有输出为静态启发式分析，不声称运行时证明

---

## 三、视图详细设计

### 3.1 Dashboard — 项目总览

**目标用户问题：** "我刚打开这个项目，它大概什么状态？"

**数据来源：**
- `codelattice_project_overview` → 统计 + qualityMetrics + diagnostics
- `codelattice_quality` → quality gates pass/fail
- `codelattice_project_insights` → 入口点、热点文件、风险区域

**关键字段：**
```json
{
  "language": "rust",
  "summary": { "sourceFileCount": 50, "symbolCount": 838, ... },
  "qualityGates": [{ "gateName": "...", "passed": true }],
  "qualityMetrics": {
    "graphCompleteness": { "danglingEdgeCount": 0 },
    "edgeConfidence": { "lowConfidenceEdgeRate": 0.0 },
    "callQuality": { "lowConfidenceCallRate": 0.0 }
  },
  "diagnosticsSummary": { "total": 1, "bySeverity": { "info": 1 } }
}
```

**必须展示的 caution / stop-line：**
- `generatedFrom.compilerVerified == false`
- 低置信度边占比（如果 > 20% 要高亮）
- dangling edge 数量
- "静态分析结果 ≠ 编译器保证"

**MVP 不做什么：**
- 不做实时监控或时间趋势图
- 不做跨项目比较
- 不做 CI/CD 集成面板

---

### 3.2 Explore — 符号探索

**目标用户问题：** "我想看某个函数的定义和调用关系"

**数据来源：**
- `codelattice_symbol_search` → 符号搜索
- `codelattice_symbol_context` → 定义位置 + 源码片段 + 出边/入边
- `codelattice_calls_from` / `codelattice_calls_to` → 调用链
- `codelattice_query_graph` → 图查询

**关键字段：**
```json
{
  "candidates": [{
    "id": "symbol:...",
    "name": "helper",
    "kind": "function",
    "file": "src/lib.rs",
    "line": 1,
    "sourceSnippet": { "lines": "pub fn helper() {...}", "startLine": 1 },
    "outgoingEdges": { "CALLS": 0 },
    "incomingEdges": { "CALLS": 1 }
  }]
}
```

**必须展示的 caution / stop-line：**
- 每条调用边的 confidence/reason
- `sourceSnippet` 可能为 null（文件读取失败）
- 符号可能有多个匹配候选（ambiguous）

**MVP 不做什么：**
- 不做 AST-level rename（只预览）
- 不做完整的 type hover / go to definition
- 不做实时代码补全

---

### 3.3 Impact — 影响分析

**目标用户问题：** "如果我改了这个函数，会出什么事？"

**数据来源：**
- `codelattice_impact_preview` → 风险等级 + 影响指标 + 审查焦点
- `codelattice_impact_analysis` → 直接/间接调用方 + 路径追踪
- `codelattice_changed_symbols` → 从 git diff 识别变更符号

**关键字段：**
```json
{
  "risk": "LOW",
  "riskReasons": ["Small blast radius, few callers"],
  "impactMetrics": {
    "callerCount": 1,
    "impactedFileCount": 1,
    "lowConfidenceEdgeCount": 0
  },
  "confidenceSummary": {
    "minConfidence": "1.00",
    "avgConfidence": "1.00"
  },
  "reviewFocus": {
    "topFiles": [...],
    "lowConfidenceEdges": []
  }
}
```

**必须展示的 caution / stop-line：**
- risk 是 graph-based preview，不是编译器级完整证明
- low-confidence edges 可能隐藏间接影响
- unknown hunk（diff 无法映射到已知符号）需要人工复核
- 动态派发/反射/插件可能隐藏实际调用方

**MVP 不做什么：**
- 不做自动 impact mitigation 建议
- 不做 what-if scenario simulation
- 不做 cross-repo dependency tracking

---

### 3.4 Cleanup — 清理辅助

**目标用户问题：** "有哪些代码可能没人在用？"

**数据来源：**
- `codelattice_dead_code_candidates` → 死代码候选
- `codelattice_reachability_map` → 入口可达性分析
- `codelattice_external_api_surface` → 公开 API 表面
- `codelattice_framework_entry_hints` → 框架入口提示

**关键字段：**
```json
{
  "deadCodeCandidates": {
    "summary": { "candidateSymbolCount": 5, "highConfidenceCount": 3 },
    "candidateSymbols": [{
      "name": "unused_fn",
      "score": 0.85,
      "confidence": "high",
      "cautions": ["static-analysis-only", "not-deletion-proof"]
    }]
  },
  "reachability": {
    "entryPoints": [...],
    "unreachableCandidates": [...]
  },
  "externalApiSurface": {
    "summary": { "externalSurfaceSymbolCount": 12 },
    "cautionLevel": "medium"
  },
  "frameworkEntryHints": {
    "frameworkEntryHintCount": 8,
    "hints": [...]
  }
}
```

**必须展示的 caution / stop-line：**
- `deletionSafe: false` — 永远不声称删除安全
- 公开 API 符号即使看起来不可达也可能有外部调用者
- framework entry hints 可能隐藏运行时调用者
- 动态派发/注册表/路由可能隐藏调用者
- **不要基于这些候选项自动删除代码**

**MVP 不做什么：**
- 不做一键删除
- 不做 refactoring suggestion generation
- 不做 test coverage correlation

---

### 3.5 Release Review — 发布审查

**目标用户问题：** "发布前有没有遗漏的风险点？"

**数据来源：**
- `codelattice_breaking_change_review` → 破坏性变更评估
- `codelattice_consistency_review` → 文档/测试一致性
- `codelattice_config_examples_review` → 配置/示例一致性

**关键字段：**
```json
{
  "breakingChangeReview": {
    "compatibilityRisk": "medium",
    "changedExternalApi": true,
    "reviewChecklist": [{ "priority": "P0", "item": "..." }]
  },
  "consistencyReview": {
    "staleDocCandidates": [...],
    "missingTestCandidates": [...],
    "coverageVerified": false
  },
  "configExamplesReview": {
    "staleExamples": [...],
    "staleConfigReferences": [...]
  }
}
```

**必须展示的 caution / stop-line：**
- `coverageVerified: false` — 不运行测试，不声称测试覆盖
- `runtimeVerified: false` — 不执行项目代码
- consistency review 基于静态文本匹配，不解析语义
- config review 不执行脚本/构建/Docker/CI
- release note hints 是建议性的，不替代人工审核

**MVP 不做什么：**
- 不做 automated changelog generation
- 不做 semantic versioning auto-bump
- 不做 release note publishing

---

## 四、信息架构原则

### 设计风格

1. **本地开发工具审美** — 信息密度高、可扫描、强调风险解释
2. **不把候选项说成事实** — 用"候选"、"可能"、"疑似"、"提示"等措辞
3. **置信度前置** — 每个诊断结论附带 confidence/reason/generatedFrom
4. **风险分级可见** — critical/high/medium/low 用颜色区分，但不制造恐慌
5. **行动导向** — 每个发现都附"建议验证步骤"，而非仅展示数据

### 数据流架构

```
项目源码 → CLI analyze (JSON) → webui-snapshot.sh (聚合) → CodeLatticeWebSnapshotV1 (JSON)
                                                                    ↓
                                              ┌─────────────────────┼─────────────────────┐
                                              ↓                     ↓                     ↓
                                          Dashboard            Explore          Impact/Cleanup/Release
```

## 五、与 MCP 的关系

| 维度 | MCP (AI channel) | WebUI (Human channel) |
|------|------------------|----------------------|
| 消费者 | AI agent / LLM | Human developer |
| 交互方式 | JSON-RPC stdio | 浏览器渲染的 JSON snapshot |
| 工具数 | 37 个独立工具 | 5 个聚合视图 |
| 实时性 | 按需调用 | 预生成 snapshot |
| 适用场景 | AI 编程助手工作流 | 人类项目理解 / 代码走查 |
| 置信度处理 | AI 自行判断 | UI 高亮 + caution banner |

---

## 七、Workspace Mode（多项目工作区）

> **Phase F — 2026-05-18**  
> 详见 [webui-workspace-smoke.sh](../../scripts/webui-workspace-smoke.sh)

Workspace Mode 允许用户选择一个包含多个项目的大目录（如 monorepo），WebUI Runner 自动发现可分析子项目和暂不支持模块。批量分析完成后会停留在 Workspace 总览，用户可以从洞察推荐项或项目评分表显式打开某个子项目 snapshot。

### 7.1 Runner API

| Endpoint | Method | 说明 |
|----------|--------|------|
| `/api/workspace/inventory?root=<path>` | GET | 扫描目录，返回支持/不支持项目列表、语言分布 |
| `/api/workspace/analyze` | POST | 批量分析（recommended/selected/all 三种模式）|
| `/api/workspace/runs` | GET | 历史 workspace run 列表 |
| `/api/workspace/run/<id>` | GET | 某次 run 详情 |
| `/api/workspace/graph?runId=<id>` | GET/POST | 跨项目关系图（节点/边） |
| `/api/workspace/insights?runId=<id>` | GET/POST | 工作区洞察（健康评分/推荐/图谱摘要） |
| `/api/workspace/impact` | GET/POST | 跨项目影响分析（BFS 遍历/风险评估/审查清单） |

### 7.2 识别规则

支持 8 种语言（Rust/Cangjie/ArkTS/TS/C/C++/Python/Shell），暂不支持但可识别 C#/Java/Go/Swift/Kotlin。只读取目录结构和文件名，不执行目标项目代码。扫描上限 depth=5、entries=5000。

### 7.3 存储与合约

Workspace run 存储在 `.codelattice-webui/workspaces/`（gitignored），不写入目标项目路径。workspaceRun 是 runner 内部状态，不改变单 snapshot schema。

### 7.4 UX 收口

- Workspace Insights 提供 Read First / Review First / Cleanup First 推荐项，并在可用时附带打开 snapshot 的操作。
- 失败项目会显示下一步建议：选择更具体子项目、显式选择语言、检查 manifest 或检查路径。
- 暂不支持模块会聚合成未来语言支持 backlog，用于后续规划，不会参与分析。
- “复制给 AI 的摘要”会生成静态分析摘要，强调这些结果不是运行时证明、测试覆盖证明或删除安全证明。
- 报告导出跟随当前 UI 语言；中文模式下输出中文 Workspace Review Report。

---

## 八、后续方向（非本轮）

- Tauri / Electron shell 包装
- 实时 MCP streaming 到 WebUI
- 符号级别 incremental update
- 跨版本 snapshot diff / compare
- 插件化 view extension system
