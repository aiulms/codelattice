# WebUI Phase A — Rich Snapshot Viewer + Export Pipeline

> **日期：** 2026-05-16
> **阶段：** Phase A (Rich Snapshot Viewer)
> **状态：** Preflight / Scope Lock
> **关联文档：** [webui-phase-a-closure.md](./2026-05-16-webui-phase-a-closure.md)

---

## 1. 目标

把 MVP 1.0（静态 viewer 壳 + 基础 snapshot contract）推进到 **Phase A：真实可浏览的 Rich Snapshot Viewer**。

用户可以对一个 Rust/TypeScript/C/C++/Python 项目生成 snapshot，打开纯静态 WebUI 后看到：
- 项目健康 Dashboard（真实 counts / quality gates / limitations）
- 真实符号/文件 Explore（source files list + symbols with details）
- 风险/质量/limitations 展示
- Cleanup 候选摘要（4 类 summary + cautions）
- Release Review 摘要（3 类 summary + cautions）
- Workflow Presets 推荐（10 个场景）
- 清晰的 static-only caution

## 2. 本轮不做什么

| 不做 | 原因 | 未来考虑 |
|------|------|----------|
| Live MCP mode | Phase A 只做静态 snapshot | Phase B |
| 后端服务 / API | 纯静态 HTML/CSS/JS | 不在当前路线图 |
| 桌面应用壳 | Tauri/Electron/HarmonyOS | Phase C |
| 图形可视化 | D3.js/cytoscape 太重 | Phase B+ |
| React/Vue/Svelte/Next/Vite/npm | 硬边界 | 永远不做 |
| 实时更新 / WebSocket | snapshot 是 point-in-time | Phase B |
| 跨项目对比 / diff | 需要 snapshot 存储 | Phase B+ |
| 直接调用长期运行 MCP server | 硬边界 | Phase B |

## 3. 核心问题诊断

MVP 1.0 snapshot 中以下 section 全部为 `not_collected`：

### 3.1 当前 not_collected 的 section

| Section | 原因 | Phase A 方案 |
|---------|------|-------------|
| `explore.symbols[]` | CLI analyze 输出包含 nodes，但 snapshot 脚本未提取 | 从 CLI analyze JSON 的 nodes 提取 symbols 列表 |
| `explore.sourceFiles[]` | 未从 nodes 中聚合 source file 信息 | 从 nodes 按 SourceFile 分组 |
| `cleanup.*` | 需要 MCP 工具 | 从 CLI analyze JSON 提取基础信息（如 unreachable candidates）或标记 heuristic summary |
| `releaseReview.*` | 需要 changed symbols list | 提供 static review summary（无 changed symbols 时显示 guidance）|
| `workflowPresets` | 未嵌入 | 内嵌 10 个预设场景定义 |
| `insights` | 需要 MCP aggregation | 从 analyze JSON 提取 entry points + basic risk map |

### 3.2 Phase A 数据增强策略

**关键洞察：** CLI `analyze --format json` 已经输出丰富的 nodes/edges 数据。Phase A 不需要调用 MCP server——只需要让 `webui-snapshot.sh` 更深入地解析 CLI 输出，提取：
1. **symbols**: 从 `nodes` 中过滤 kind=symbol 的节点
2. **sourceFiles**: 从 `nodes` 中按 SourceFile 节点 + OWNS_SOURCE 关系聚合
3. **basic cleanup hints**: 从 graph 结构推断（如无 incoming CALLS 的符号）
4. **workflowPresets**: 静态内嵌 10 个预设
5. **insights**: 入口点检测 + 热点文件

## 4. 新增 CLI 参数

```bash
# 现有参数保持兼容
--root <path>
--language <lang|auto>
--output <path|->
--compact

# 新增参数
--include-explore      # 提取 explore 数据（symbols + source files）
--include-review       # 提取 cleanup/releaseReview 基础摘要
--include-workflows    # 嵌入 workflow presets
--full                 # 等价于 --include-explore --include-review --include-workflows
--redact-root          # 将绝对路径替换为 <redacted-root>
```

默认行为变更：**默认启用 `--full` 行为**（即对 fixture/小项目自动提取 explore/review/workflow）。对超大项目可用 `--compact` 减少输出。

## 5. Snapshot Enrichment 规格

### 5.1 explore（从 CLI analyze 提取）

```json
{
  "explore": {
    "status": "collected",
    "sourceFiles": [
      {
        "path": "src/lib.rs",
        "language": "rust",
        "symbolCount": 5,
        "edgeCount": 8
      }
    ],
    "searchMeta": {
      "totalSymbols": 9,
      "availableKinds": ["function", "struct", "enum", "trait", "impl"]
    },
    "symbols": [
      {
        "id": "node:xxx",
        "name": "helper",
        "kind": "function",
        "file": "src/lib.rs",
        "line": 1,
        "visibility": "private",
        "outgoingEdges": { "CALLS": 2 },
        "incomingEdges": { "CALLS": 1 }
      }
    ]
  }
}
```

数据来源：CLI `analyze --format json` -> `nodes[]` (filter kind=Symbol) + `edges[]`

### 5.2 cleanup（heuristic summary from CLI data）

即使没有 MCP 工具，也能从 graph 结构提供基础信息：

```json
{
  "cleanup": {
    "deadCodeCandidates": {
      "status": "collected",
      "reason": "heuristic: based on static call graph analysis only",
      "summary": { "candidateSymbolCount": 2, ... },
      "candidateSymbols": [...],
      "deletionSafe": false
    },
    "reachability": {
      "status": "collected",
      "reason": "heuristic: BFS from detected entry points",
      "summary": { "entryPointCount": 1, "unreachableCandidateCount": 2 },
      "warnings": ["Static graph reachability only"]
    },
    "externalApiSurface": { "status": "not_collected", "reason": "..." },
    "frameworkEntries": { "status": "not_collected", "reason": "..." }
  }
}
```

### 5.3 releaseReview（guidance mode）

没有 changed symbols 时提供 static review guidance：

```json
{
  "releaseReview": {
    "breakingChange": { "status": "partial", "reason": "no changed symbols; showing guidance" },
    "consistency": { "status": "partial", "reason": "static file scan only" },
    "configExamples": { "status": "partial", "reason": "pattern-based scan only" }
  }
}
```

### 5.4 workflowPresets（内嵌 10 场景）

```json
{
  "workflowPresets": {
    "status": "collected",
    "presets": [
      { "scenario": "onboarding", "description": "...", "steps": [...] },
      { "scenario": "before_edit", "description": "...", "steps": [...] },
      { "scenario": "after_edit", "description": "...", "steps": [...] },
      { "scenario": "delete_code", "description": "...", "steps": [...] },
      { "scenario": "release_check", "description": "...", "steps": [...] },
      { "scenario": "legacy_cleanup", "description": "...", "steps": [...] },
      { "scenario": "public_api_change", "description": "...", "steps": [...] },
      { "scenario": "framework_route_change", "description": "...", "steps": [...] },
      { "scenario": "docs_tests_sync", "description": "...", "steps": [...] },
      { "scenario": "config_examples_sync", "description": "...", "steps": [...] }
    ],
    "presetOnly": true
  }
}
```

## 6. Multi-Language Fixture Matrix

| 语言 | Fixture Path | 必须生成 | 已有 fixture |
|------|-------------|---------|-------------|
| Rust | `fixtures/rust/portable-smoke` | YES | YES |
| TypeScript | `fixtures/typescript/portable-smoke` | YES | YES |
| C | `fixtures/c/` | YES | YES (14 files) |
| C++ | `fixtures/cpp/` | YES | YES (14 files) |
| Python | `fixtures/python/` | YES | YES (20 files) |
| ArkTS | `fixtures/arkts/` | 可选 | YES |
| Cangjie | `fixtures/cangjie/` | 可选 | YES |

每个 snapshot 必须：
- schemaVersion == webui.snapshot.v1
- generatedFrom.staticAnalysis == true
- sourceFileCount > 0 && symbolCount > 0
- explore.status == collected 且 symbols 非空
- workflowPresets.status == collected
- 使用 --redact-root 无绝对路径

## 7. Viewer 渲染升级规格

### 7.1 Dashboard 增强
- 增加 Node Count / Edge Count 统计卡片 (共 6 卡片)
- Quality metrics 子面板 (dangling edge count, low confidence rate)
- Insights entry points 列表

### 7.2 Explore 增强
- Source Files panel (新增左侧 tab 或顶部切换)
- Symbols 增强: visibility badge, sort 功能, 更多 detail
- 符号列表和详情面板保持原有布局

### 7.3 Cleanup 增强
- deadCodeCandidates: 显示 candidate symbols 列表
- reachability: 显示 entry points + unreachable candidates
- externalApiSurface/frameworkEntries: not_collected 但清晰引导

### 7.4 Release Review 增强
- breakingChange: 显示 guidance checklist
- consistency: 显示 docs/tests 文件统计 (from docsTestsConfig)
- configExamples: 显示配置文件列表 (from docsTestsConfig)

### 7.5 Workflow Presets
- 新增第 6 个 tab "Workflows"
- 展示 10 个 preset 卡片: 场景名、描述、工具链摘要、stop-line

### 7.6 视觉要求
- 保持本地开发工具风格
- 信息密度高、可扫描
- app.js 如果超过 700 行考虑拆分

## 8. Stop-lines

1. dead-code candidate 不可说成可删除
2. external API heuristic 不可说成真实外部使用
3. quality/release review 不可说成 GA proof
4. 不执行项目代码
5. 所有 caution banner 必须保留
6. heuristic/preview 字段必须有视觉标识

## 9. Acceptance Criteria

1. Rust/TS/C/C++/Python fixture snapshots 都能生成并打开
2. Dashboard 显示真实 counts / quality / limitations
3. Explore 有真实 source files 和 symbols
4. Cleanup 有 deadCodeCandidates + reachability summary
5. Release Review 有 guidance mode 内容
6. Workflow presets 有 10 个可见推荐
7. Smoke 可验证核心数据入口
8. 所有 caution/heuristic 正确渲染
9. --redact-root 正确工作
10. 不触碰外部 repo / AI config / 真实项目
