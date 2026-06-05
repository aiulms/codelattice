# CodeLattice WebUI Snapshot Contract — V1

> **日期：** 2026-05-18
> **版本：** `webui.snapshot.v1`
> **状态：** Active（WebUI Phase F — Workspace Intelligence Pack 已落地）
> **关联文档：** [README.md](./README.md)、[webui-mvp.md](./webui-mvp.md)
>
> **Workspace 说明：** `workspaceRun` 是 WebUI Runner 内部状态，存储在 `.codelattice-webui/workspaces/` 下，不强行改变单 `CodeLatticeWebSnapshotV1` 的 schema。Workspace analyze 会为每个子项目生成独立 snapshot，最后在 workspace run JSON 中聚合结果。

---

## 一、Contract 概览

### 1.1 目的

定义 `CodeLatticeWebSnapshotV1` JSON 结构——一个**只读、可缓存、可导出、可比较**的聚合数据快照。

WebUI 前端（未来实现）只消费这个 snapshot JSON，不需要理解 37 个 MCP 工具的输入输出契约。

### 1.2 设计原则

| 原则 | 描述 |
|------|------|
| **Read-only** | snapshot 不修改项目源码 |
| **Cacheable** | 相同 root + language + mtime → 可复用 |
| **Exportable** | 单个 JSON 文件，可用 `python3 -m json.tool` 验证 |
| **Comparable** | 两个 snapshot 可做字段级 diff |
| **Stable contract** | stable 字段不删不改类型；preview/heuristic 字段可能演进 |
| **No runtime proof** | `generatedFrom` 明确标注静态分析限制 |

### 1.3 生成方式

```bash
bash scripts/webui-snapshot.sh \
  --root <project-path> \
  --language rust|cangjie|arkts|typescript|c|cpp|python|auto \
  --output /path/to/snapshot.json
```

或 stdout：

```bash
bash scripts/webui-snapshot.sh --root . --language rust --output -
```

---

## 二、顶层结构

```json
{
  "schemaVersion": "webui.snapshot.v1",
  "generatedAt": "2026-05-16T23:00:00+08:00",
  "generatorVersion": "0.17.0-beta.1",
  "root": "<project-root>",
  "language": "rust",
  "generatedFrom": {
    "staticAnalysis": true,
    "runtimeVerified": false,
    "externalUsageVerified": false,
    "coverageVerified": false,
    "deletionSafetyVerified": false
  },
  "summary": {},
  "quality": {},
  "insights": {},
  "explore": {},
  "impact": {},
  "cleanup": {},
  "releaseReview": {},
  "docsTestsConfig": {},
  "workflowPresets": {},
  "limitations": []
}
```

### 2.1 元数据字段 (Metadata)

| 字段 | 类型 | 稳定性 | 说明 |
|------|------|--------|------|
| `schemaVersion` | string | **stable** | 固定值 `"webui.snapshot.v1"` |
| `generatedAt` | string | **stable** | ISO 8601 时间戳 |
| `generatorVersion` | string | **stable** | CodeLattice 产品版本号 |
| `root` | string | **stable** | 项目根路径（snapshot 内可为相对占位） |
| `language` | string | **stable** | 检测/指定的语言标识 |
| `generatedFrom` | object | **static** | 分析来源标注（见下表） |

#### generatedFrom 子字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `staticAnalysis` | boolean | true | 是否来自静态分析 |
| `runtimeVerified` | boolean | **false** | 是否有运行时验证（永远为 false） |
| `externalUsageVerified` | boolean | **false** | 是否验证了外部使用（永远为 false） |
| `coverageVerified` | boolean | **false** | 是否有测试覆盖证明（永远为 false） |
| `deletionSafetyVerified` | boolean | **false** | 是否有删除安全性证明（永远为 false） |

> **Invariant**: `runtimeVerified`, `externalUsageVerified`, `coverageVerified`, `deletionSafetyVerified` 在 V1 中**永远为 false**。如果未来某版本变为 true，意味着 schema major version 升级。

---

## 三、Section 详细定义

### 3.1 summary — 项目概览统计

**来源 MCP 工具:** `codelattice_analyze`, `codelattice_project_overview`, `codelattice_summary`

**稳定性:** stable

```json
{
  "summary": {
    "nodeCount": 1524,
    "edgeCount": 2438,
    "symbolCount": 838,
    "sourceFileCount": 50,
    "packageCount": 3,
    "diagnosticCount": 1,
    "callEdgeCount": 1054,
    "topNodeKinds": [
      { "kind": "symbol", "count": 838 },
      { "kind": "source-file", "count": 50 },
      { "kind": "package", "count": 3 }
    ],
    "topEdgeKinds": [
      { "kind": "CALLS", "count": 1054 },
      { "kind": "DEFINES", "count": 838 },
      { "kind": "OWNS_SOURCE", "count": 50 }
    ]
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `nodeCount` | integer | ✅ | 图节点总数 |
| `edgeCount` | integer | ✅ | 图边总数 |
| `symbolCount` | integer | ✅ | 符号总数 |
| `sourceFileCount` | integer | ✅ | 源码文件数 |
| `packageCount` | integer | ✅ | 包/模块总数 |
| `diagnosticCount` | integer | ✅ | 诊断条目数 |
| `callEdgeCount` | integer | ✅ | 调用边数量 |
| `topNodeKinds` | array | ✅ | 节点类型分布 top-N |
| `topEdgeKinds` | array | ✅ | 边类型分布 top-N |

---

### 3.2 quality — 质量门与质量指标

**来源 MCP 工具:** `codelattice_quality`, `codelattice_project_overview`(qualityMetrics)

**稳定性:** stable

```json
{
  "quality": {
    "overall": "pass",
    "totalGates": 7,
    "passedGates": 7,
    "failedGates": 0,
    "gates": [
      {
        "gateName": "duplicate_nodes",
        "passed": true,
        "detail": "0 duplicate node IDs found"
      },
      {
        "gateName": "dangling_edges",
        "passed": true,
        "detail": "0 dangling edge targets"
      },
      {
        "gateName": "graph_integrity",
        "passed": true,
        "detail": "all endpoints resolve to existing nodes"
      }
    ],
    "metrics": {
      "graphCompleteness": {
        "nodeCount": 1524,
        "edgeCount": 2438,
        "symbolCount": 838,
        "sourceFileCount": 50,
        "danglingEdgeCount": 0
      },
      "edgeConfidence": {
        "totalConfidenceEdgeCount": 2438,
        "highConfidenceEdgeCount": 2200,
        "mediumConfidenceEdgeCount": 200,
        "lowConfidenceEdgeCount": 38,
        "unknownConfidenceEdgeCount": 0,
        "lowConfidenceEdgeRate": 0.0156,
        "unknownConfidenceEdgeRate": 0.0
      },
      "callQuality": {
        "callEdgeCount": 1054,
        "highConfidenceCallEdgeCount": 1000,
        "mediumConfidenceCallEdgeCount": 40,
        "lowConfidenceCallEdgeCount": 14,
        "unknownConfidenceCallEdgeCount": 0,
        "lowConfidenceCallRate": 0.0133
      },
      "dependencyQuality": {
        "importEdgeCount": 120,
        "includeEdgeCount": 30,
        "unresolvedImportOrIncludeCount": 2
      },
      "diagnostics": {
        "diagnosticCount": 1,
        "unresolvedDiagnosticCount": 0,
        "parseDiagnosticCount": 0
      }
    },
    "diagnosticsSummary": {
      "total": 1,
      "bySeverity": { "info": 1, "warning": 0, "error": 0 }
    }
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `overall` | string | ✅ | `pass` / `fail` / `warn` |
| `gates[]` | array | ✅ | 每个 quality gate 的 pass/fail 详情 |
| `metrics` | object | ✅ | 统一质量指标对象 |
| `diagnosticsSummary` | object | ✅ | 诊断严重级别分布 |

---

### 3.3 insights — 项目洞察地图

**来源 MCP 工具:** `codelattice_project_insights`

**稳定性:** heuristic（评分和分类依赖启发式算法，可能随版本调整）

```json
{
  "insights": {
    "status": "collected",
    "entryPoints": [
      {
        "id": "sym_main_1",
        "name": "main",
        "kind": "function",
        "file": "src/main.rs",
        "line": 1,
        "confidence": 0.95,
        "reasons": ["main-function-naming"]
      }
    ],
    "readFirst": [
      { "file": "src/lib.rs", "reason": "highest-symbol-count" },
      { "file": "src/main.rs", "reason": "entry-point" }
    ],
    "reviewFirst": [
      { "file": "src/parser.rs", "reason": "high-fan-out-hotspot" }
    ],
    "riskMap": {
      "overallRisk": "MEDIUM",
      "hotspotFiles": [
        { "file": "src/engine.rs", "symbolCount": 45, "edgeDensity": "high" }
      ],
      "lowConfidenceZones": [
        { "file": "src/dynamic_dispatch.rs", "reason": "heavy-trait-object-usage" }
      ]
    },
    "recommendations": [
      "Review src/engine.rs for coupling reduction opportunities",
      "Investigate src/dynamic_dispatch.rs for low-confidence call edges"
    ]
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `status` | string | ✅ | `collected` / `not_collected` / `partial` |
| `entryPoints[]` | array | ✅ | 检测到的入口点列表 |
| `readFirst[]` | array | 建议 | 接手项目时优先阅读的文件 |
| `reviewFirst[]` | array | 建议 | 改代码前先审查的文件 |
| `riskMap` | object | 建议 | 风险总览（含热点文件和低置信度区域） |
| `recommendations[]` | array | 建议 | 文本建议 |

---

### 3.4 explore — 符号探索数据

**来源 MCP 工具:** `codelattice_symbol_search`, `codelattice_symbol_context`, `codelattice_query_graph`

**稳定性:** stable（核心字段）/ preview（搜索元数据）

```json
{
  "explore": {
    "status": "collected",
    "searchMeta": {
      "totalSymbols": 838,
      "availableKinds": ["function", "struct", "enum", "trait", "impl", "method", "const", "static", "macro", "class", "interface", "type", "variable"]
    },
    "symbols": [
      {
        "id": "symbol:c1::crate::helper",
        "name": "helper",
        "kind": "function",
        "file": "src/lib.rs",
        "line": 1,
        "lineEnd": 3,
        "visibility": "public",
        "sourceSnippet": {
          "lines": "pub fn helper() -> i32 {\n    42\n}\n",
          "startLine": 1,
          "endLine": 3,
          "totalLines": 10
        },
        "outgoingEdges": { "CALLS": 2, "ACCESSES": 0 },
        "incomingEdges": { "CALLS": 5, "DEFINES": 1 },
        "relatedDiagnostics": 0,
        "confidenceSamples": [
          { "confidence": 0.9, "reason": "call-same-module-resolved" }
        ]
      }
    ]
  }
}
```

注意：`explore.symbols[]` 在 MVP 中可以是一个**代表性子集**（如 top-50 by fan-in），不需要包含全部符号。完整符号列表可通过 MCP `query_graph` 或 `symbol_search` 按需获取。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `status` | string | ✅ | `collected` / `not_collected` |
| `searchMeta.totalSymbols` | integer | ✅ | 项目中符号总数 |
| `searchMeta.availableKinds` | array | ✅ | 可用的符号类型列表 |
| `symbols[]` | array | ✅ | 符号列表（MVP 可为子集） |

---

### 3.5 impact — 影响分析数据

**来源 MCP 工具:** `codelattice_impact_preview`, `codelattice_impact_analysis`

**稳定性:** stable

```json
{
  "impact": {
    "status": "not_collected",
    "reason": "requires target symbol parameter; use MCP impact_preview for on-demand analysis",
    "sampleEntries": []
  }
}
```

> **设计决策:** Impact 分析是按需的（需要指定目标 symbol），不适合预聚合到 snapshot 中。Snapshot 的 impact section 默认为 `not_collected`，但保留结构供未来 WebUI 做 on-demand MCP 调用后填充。

如果 snapshot 生成时指定了 `--impact-symbols` 参数：

```json
{
  "impact": {
    "status": "collected",
    "entries": [
      {
        "targetSymbol": "helper",
        "targetId": "sym_helper_1",
        "risk": "LOW",
        "riskLevel": "low",
        "riskReasons": ["Small blast radius, few callers"],
        "impactedNodeCount": 2,
        "impactedFileCount": 1,
        "directCallers": [
          { "name": "main_fn", "file": "src/main.rs", "confidence": 0.9, "reason": "call-same-module-resolved" }
        ],
        "directCallees": [],
        "impactMetrics": {
          "callerCount": 1,
          "downstreamCount": 0,
          "crossFileCount": 0,
          "publicSymbolCount": 0,
          "testFileCount": 0,
          "lowConfidenceEdgeCount": 0
        },
        "confidenceSummary": {
          "minConfidence": "1.00",
          "avgConfidence": "1.00",
          "maxConfidence": "1.00"
        },
        "reviewFocus": {
          "topFiles": [{ "file": "src/lib.rs", "impactedNodeCount": 2 }],
          "lowConfidenceEdges": []
        }
      }
    ]
  }
}
```

---

### 3.6 cleanup — 清理辅助数据

**来源 MCP 工具:** `codelattice_dead_code_candidates`, `codelattice_reachability_map`, `codelattice_external_api_surface`, `codelattice_framework_entry_hints`

**稳定性:** heuristic（所有子字段都依赖启发式分析）

```json
{
  "cleanup": {
    "deadCodeCandidates": {
      "status": "collected",
      "summary": {
        "candidateSymbolCount": 5,
        "candidateFileCount": 2,
        "highConfidenceCount": 3,
        "mediumConfidenceCount": 2,
        "lowConfidenceCount": 0,
        "publicApiCautionCount": 1,
        "dynamicFeatureCautionCount": 0
      },
      "candidateSymbols": [
        {
          "id": "sym_unused_1",
          "name": "unused_function",
          "kind": "function",
          "file": "src/legacy.rs",
          "line": 10,
          "score": 0.85,
          "confidence": "high",
          "cautions": ["static-analysis-only", "not-deletion-proof"],
          "recommendedVerification": ["Check for test usage", "Search for runtime reflection calls"]
        }
      ],
      "candidateFiles": [
        {
          "path": "src/deprecated_module.rs",
          "score": 0.78,
          "confidence": "medium",
          "cautions": ["may-contain-public-re-exports"]
        }
      ],
      "deletionSafe": false
    },
    "reachability": {
      "status": "collected",
      "summary": {
        "entryPointCount": 3,
        "reachableSymbolCount": 800,
        "unreachableCandidateCount": 38,
        "reachableFileCount": 48,
        "totalFiles": 50
      },
      "entryPoints": [
        { "name": "main", "kind": "function", "file": "src/main.rs", "confidence": 0.95 }
      ],
      "unreachableCandidates": [
        {
          "id": "sym_unreach_1",
          "name": "orphaned_util",
          "kind": "function",
          "file": "src/utils.rs",
          "confidence": "medium",
          "cautions": ["may-be-called-via-dynamic-dispatch"]
        }
      ],
      "warnings": [
        "Static graph reachability only — dynamic dispatch may hide runtime reachability",
        "Cross-crate callers not visible without workspace-level analysis"
      ]
    },
    "externalApiSurface": {
      "status": "collected",
      "summary": {
        "externalSurfaceSymbolCount": 12,
        "externalSurfaceFileCount": 5,
        "averageCautionScore": 0.62,
        "highCautionCount": 3,
        "mediumCautionCount": 6,
        "lowCautionCount": 3
      },
      "surfaceSymbols": [
        {
          "name": "createClient",
          "kind": "function",
          "file": "src/public_api.rs",
          "line": 4,
          "cautionLevel": "high",
          "score": 0.85,
          "reasons": ["public-visibility", "lib.rs-exported", "documented-in-readme"]
        }
      ]
    },
    "frameworkEntries": {
      "status": "collected",
      "summary": {
        "frameworkEntryHintCount": 8,
        "routeHintCount": 4,
        "callbackHintCount": 2,
        "componentHintCount": 1,
        "cliHintCount": 1
      },
      "hints": [
        {
          "name": "getUserHandler",
          "kind": "function",
          "file": "src/routes/users.rs",
          "hintKind": "route",
          "framework": "express/nextjs",
          "score": 0.75,
          "confidence": "medium",
          "cautions": ["framework-callback-may-hide-callers"]
        }
      ]
    }
  }
```

---

### 3.7 releaseReview — 发布审查数据

**来源 MCP 工具:** `codelattice_breaking_change_review`, `codelattice_consistency_review`, `codelattice_config_examples_review`, `codelattice_automation_graph`

**稳定性:** heuristic（review checklist 结构稳定，具体条目是启发式的）

```json
{
  "releaseReview": {
    "breakingChange": {
      "status": "not_collected",
      "reason": "requires changed symbols list; use MCP breaking_change_review for on-demand analysis",
      "compatibilityRisk": null,
      "reviewChecklist": []
    },
    "consistency": {
      "status": "not_collected",
      "reason": "requires changed symbols list; use MCP consistency_review for on-demand analysis",
      "staleDocCandidates": [],
      "missingDocCandidates": [],
      "relatedTests": [],
      "missingTestCandidates": [],
      "staleTestCandidates": []
    },
    "configExamples": {
      "status": "not_collected",
      "reason": "requires changed symbols list; use MCP config_examples_review for on-demand analysis",
      "staleExamples": [],
      "staleConfigReferences": [],
      "packageScriptRisks": []
    }
  }
```

> 同 impact section，release review 默认 `not_collected`。当提供 `--changed-symbols` 参数时可填充。

---

### 3.8 automationGraph — 自动化图谱审查

**来源 MCP 工具:** `codelattice_automation_graph`

**稳定性:** heuristic（风险候选是静态审查线索，不执行脚本）

```json
{
  "automationGraph": {
    "summary": {
      "workflowCount": 4,
      "stepCount": 18,
      "riskCount": 2,
      "highRiskCount": 1,
      "staticOnly": true
    },
    "workflows": [],
    "riskFindings": [],
    "generatedFrom": {
      "staticAnalysis": true,
      "scriptsExecuted": false,
      "buildExecuted": false,
      "runtimeVerified": false
    }
  }
}
```

Runner 模式会 best-effort 填充该 section。MCP 不可用时允许：

```json
{ "automationGraph": { "status": "not_collected", "reason": "mcp_unavailable", "staticOnly": true } }
```

---

### 3.9 docsTestsConfig — 文档/测试/配置索引

**来源 MCP 工具:** `codelattice_project_overview`(docs field), 静态文件扫描

**稳定性:** preview

```json
{
  "docsTestsConfig": {
    "status": "collected",
    "docs": {
      "docCount": 12,
      "docSectionCount": 45,
      "docLinkCount": 30,
      "docSymbolReferenceCount": 18,
      "topDocPaths": ["README.md", "docs/architecture/mcp-v0-contract.md"]
    },
    "tests": {
      "testFileCount": 0,
      "testFrameworksDetected": []
    },
    "configFiles": {
      "paths": ["Cargo.toml", ".gitignore", "rustfmt.toml"]
    }
  }
}
```

---

### 3.10 workflowPresets — 工作流预设

**来源 MCP 工具:** `codelattice_workflow_presets`

**稳定性:** stable（preset 定义不变）

```json
{
  "workflowPresets": {
    "status": "collected",
    "presets": [
      {
        "scenario": "onboarding",
        "description": "接手陌生项目的推荐工具链",
        "steps": [
          { "tool": "codelattice_project_insights", "inspect": "readFirst,riskMap,lowConfidenceZones" },
          { "tool": "codelattice_reachability_map", "inspect": "entryPoints,unreachableCandidates" },
          { "tool": "codelattice_external_api_surface", "inspect": "externalSurfaceSymbols,cautionLevel" },
          { "tool": "codelattice_framework_entry_hints", "inspect": "frameworkEntryHints,hintKind" },
          { "tool": "codelattice_review_plan", "params": { "mode": "onboarding" } }
        ]
      }
    ],
    "presetOnly": true,
    "analysisExecuted": false
  }
}
```

---

### 3.10 limitations — 已知限制清单

**来源:** CodeLattice stop-lines + 语言特定限制

**稳定性:** stable（结构固定；内容随语言变化）

```json
{
  "limitations": [
    "Static graph analysis only — no runtime behavior proof",
    "No full type inference or trait solving",
    "No macro expansion or proc-macro execution",
    "Call edges are heuristic with confidence scores — not compiler-verified",
    "Dynamic dispatch / reflection / plugins may hide actual callers",
    "Dead-code candidates are NOT deletion-proof — always verify manually",
    "External API surface is NOT external-usage-verified",
    "Consistency review does not run tests or execute scripts",
    "Config/examples review does not build Docker images or run CI",
    "Cross-crate/cross-package dependencies may be incomplete",
    "Language-specific limits apply — see language-specific items below"
  ]
}
```

---

## 四、最小样例 (Minimal Example)

适用于最小 fixture（如 portable-smoke）的精简 snapshot：

```json
{
  "schemaVersion": "webui.snapshot.v1",
  "generatedAt": "2026-05-16T23:00:00+08:00",
  "generatorVersion": "0.17.0-beta.1",
  "root": "<fixture-root>",
  "language": "rust",
  "generatedFrom": {
    "staticAnalysis": true,
    "runtimeVerified": false,
    "externalUsageVerified": false,
    "coverageVerified": false,
    "deletionSafetyVerified": false
  },
  "summary": {
    "nodeCount": 7,
    "edgeCount": 6,
    "symbolCount": 2,
    "sourceFileCount": 1,
    "packageCount": 2,
    "diagnosticCount": 1,
    "callEdgeCount": 1
  },
  "quality": {
    "overall": "pass",
    "totalGates": 7,
    "passedGates": 7,
    "failedGates": 0,
    "gates": [],
    "metrics": {
      "graphCompleteness": { "danglingEdgeCount": 0 },
      "edgeConfidence": { "lowConfidenceEdgeRate": 0.0 },
      "callQuality": { "lowConfidenceCallRate": 0.0 }
    },
    "diagnosticsSummary": { "total": 1, "bySeverity": { "info": 1 } }
  },
  "insights": { "status": "collected", "entryPoints": [], "readFirst": [], "reviewFirst": [], "riskMap": {}, "recommendations": [] },
  "explore": { "status": "collected", "searchMeta": { "totalSymbols": 2, "availableKinds": [] }, "symbols": [] },
  "impact": { "status": "not_collected", "reason": "requires target symbol parameter", "sampleEntries": [] },
  "cleanup": {
    "deadCodeCandidates": { "status": "not_collected", "reason": "fixture too small for meaningful dead-code detection" },
    "reachability": { "status": "not_collected", "reason": "fixture too small for meaningful reachability analysis" },
    "externalApiSurface": { "status": "not_collected", "reason": "no public API surface in fixture" },
    "frameworkEntries": { "status": "not_collected", "reason": "no framework patterns in fixture" }
  },
  "releaseReview": {
    "breakingChange": { "status": "not_collected", "reason": "no changed symbols provided" },
    "consistency": { "status": "not_collected", "reason": "no changed symbols provided" },
    "configExamples": { "status": "not_collected", "reason": "no changed symbols provided" }
  },
  "docsTestsConfig": { "status": "collected", "docs": { "docCount": 0 }, "tests": { "testFileCount": 0 }, "configFiles": { "paths": [] } },
  "workflowPresets": {
    "status": "collected",
    "presets": [{ "scenario": "onboarding", "description": "...", "steps": [] }],
    "presetOnly": true
  },
  "limitations": [
    "Static graph analysis only — no runtime behavior proof",
    "No macro expansion or proc-macro execution",
    "Dead-code candidates are NOT deletion-proof",
    "Snapshot is a point-in-time aggregate; run fresh for updated results"
  ]
}
```

---

## 五、完整样例结构 (Full Schema Skeleton)

展示所有可能的非空值字段（实际值取决于具体项目）：

```json
{
  "schemaVersion": "webui.snapshot.v1",
  "generatedAt": "2026-05-16T23:00:00+08:00",
  "generatorVersion": "0.17.0-beta.1",
  "root": "<project-root>",
  "language": "rust",
  "generatedFrom": {
    "staticAnalysis": true,
    "runtimeVerified": false,
    "externalUsageVerified": false,
    "coverageVerified": false,
    "deletionSafetyVerified": false
  },
  "summary": { "/* 见 3.1 */ },
  "quality": { "/* 见 3.2 */ },
  "insights": { "/* 见 3.3 */ },
  "explore": { "/* 见 3.4 */ },
  "impact": { "/* 见 3.5 */ },
  "cleanup": {
    "deadCodeCandidates": { "/* 见 3.6 */ },
    "reachability": { "/* 见 3.6 */ },
    "externalApiSurface": { "/* 见 3.6 */ },
    "frameworkEntries": { "/* 见 3.6 */ }
  },
  "releaseReview": {
    "breakingChange": { "/* 见 3.7 */ },
    "consistency": { "/* 见 3.7 */ },
    "configExamples": { "/* 见 3.7 */ }
  },
  "docsTestsConfig": { "/* 见 3.8 */ },
  "workflowPresets": { "/* 见 3.9 */ },
  "limitations": [ "/* 见 3.10 */" ]
}
```

---

## 六、字段稳定性总结

| Section | 核心稳定性 | 说明 |
|---------|-----------|------|
| `schemaVersion` | **stable** | V1 期间不变 |
| `generatedFrom` | **stable** | V1 期间布尔值语义不变 |
| `summary` | **stable** | 计数字段不会改名 |
| `quality` | **stable** | gates 列表会扩展但不会删除已有 gate 名 |
| `insights` | **heuristic** | entryPoints 检测算法可能改进，排序可能变 |
| `explore` | **stable** | 符号核心 id/name/kind/file/line 不变 |
| `impact` | **stable** | risk/riskReasons/metrics 结构不变 |
| `cleanup.*` | **heuristic** | 所有 cleanup 子 section 都是启发式评分 |
| `releaseReview.*` | **heuristic** | review checklist 条目是动态生成的 |
| `docsTestsConfig` | **preview** | 结构可能根据反馈调整 |
| `workflowPresets` | **stable** | preset 定义只在新增 scenario 时扩展 |
| `limitations` | **stable** | 只增不减 |

---

## 七、Snapshot 属性

| 属性 | 值 | 说明 |
|------|-----|------|
| **Read-only** | 是 | 不修改任何源文件 |
| **Cacheable** | 是 | root + language + mtime 相同可缓存 |
| **Exportable** | 是 | 单一 JSON 文件 |
| **Comparable** | 是 | 两个 snapshot 可逐字段 diff |
| **Deterministic** | 部分 | 同一输入产生相同输出（generatedAt 除外） |
| **Portable** | 是 | 不含绝对路径（root 用占位符或相对路径） |
| **Executable-safe** | 是 | 不执行项目代码 |

---

## 八、生成命令记录

### Rust fixture snapshot

```bash
bash scripts/webui-snapshot.sh \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --output fixtures/webui-snapshots/rust-portable-smoke.snapshot.json
```

### TypeScript fixture snapshot

```bash
bash scripts/webui-snapshot.sh \
  --root fixtures/typescript/portable-smoke \
  --language typescript \
  --output fixtures/webui-snapshots/typescript-portable-smoke.snapshot.json
```

### 自定义项目 snapshot

```bash
bash scripts/webui-snapshot.sh \
  --root /path/to/your/project \
  --language auto \
  --output /tmp/my-project-snapshot.json

# stdout 输出：
bash scripts/webui-snapshot.sh \
  --root /path/to/your/project \
  --language auto \
  --output -

# compact 模式（减少空白）：
bash scripts/webui-snapshot.sh \
  --root /path/to/your/project \
  --language auto \
  --output /tmp/snapshot.json \
  --compact
```
