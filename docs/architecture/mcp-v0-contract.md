# MCP Contract — CodeLattice AI Layer

> **日期：** 2026-05-15
> **版本：** v0.13.0
> **状态：** Active (MCP v0.13 Persistent Cache Pack)
> **定位：** AI agent 可通过 MCP JSON-RPC 调用 CodeLattice CLI 的分析/质量/概要/Smoke/查询/导出/本地图谱智能/两层缓存（memory + persistent）/指纹过期检测/stale reason/源码片段/production assist/compare runs/Code ↔ Docs Association 能力

---

## 一、定位

MCP v0 是 CodeLattice CLI 的 thin stdio wrapper：

- **Read-only** — 只读项目分析，不写源码
- **Multi-language** — 支持 Rust、Cangjie、ArkTS、TypeScript、C++
- **Not GitNexus-RC replacement** — 不替代 GitNexus-RC MCP server
- **Not default tool switch** — 显式 opt-in
- **Persistent cache opt-in** — 通过 `CODELATTICE_CACHE_DIR` 环境变量启用持久化分析缓存
- **No Cypher parser** — query_graph 仅支持参数化查询，不接受任意查询字符串
- **No rename apply** — rename_preview 只预览，不写文件
- **No cross-repo** — 不做跨仓库语义边

---

## 二、Transport

**Newline-delimited JSON-RPC over stdio**

- Input: stdin, one JSON-RPC request per line
- Output: stdout, one JSON-RPC response per line
- Logging: stderr only (never stdout)

### 2.1 JSON-RPC Methods

| Method | Direction | Purpose |
|--------|-----------|---------|
| `initialize` | client → server | Handshake, return capabilities |
| `notifications/initialized` | client → server | Client ready notification (no response) |
| `tools/list` | client → server | Return available tools |
| `tools/call` | client → server | Execute a tool |
| `shutdown` | client → server | Graceful termination |

---

## 三、Tools

### 3.1 `codelattice_analyze`

分析指定项目，返回 graph summary、quality gates、diagnostics summary。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "strict": { "type": "boolean", "default": true, "description": "质量门失败时标记为错误" },
    "includeGraph": { "type": "boolean", "default": false, "description": "是否包含完整 graph（默认关闭以减少 token）" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "analyzedAt": "2026-05-10T12:00:00Z",
  "schemaVersion": "v0.3",
  "summary": { "nodeCount": 1524, "edgeCount": 2438, "symbolCount": 838, "sourceFileCount": 50, "packageCount": 3, "diagnosticCount": 0, "callEdgeCount": 1054 },
  "qualityGates": [ { "gateName": "duplicate_nodes", "passed": true, "detail": "0 duplicate node IDs found" } ],
  "diagnosticsSummary": { "totalErrors": 0, "totalWarnings": 0 },
  "graph": null
}
```

### 3.2 `codelattice_quality`

运行质量门检查，返回每个 gate 的 pass/fail。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "overall": "pass",
  "gates": [ { "gateName": "duplicate_nodes", "passed": true, "detail": "..." } ]
}
```

> **v0.1 变更**: failed gates 现在排在 passed gates 前面，便于 AI 快速定位问题。

### 3.3 `codelattice_summary`

返回紧凑的 graph stats + quality summary（不含完整 graph）。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "graphSummary": { "nodeCount": 1524, "edgeCount": 2438, "symbolCount": 838, "sourceFileCount": 50, "packageCount": 3, "diagnosticCount": 0, "callEdgeCount": 1054 },
  "qualitySummary": { "total": 7, "passed": 7, "failed": 0 }
}
```

### 3.4 `codelattice_smoke`

运行端到端 smoke 测试。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "mode": { "type": "string", "enum": ["rust-only", "cangjie-only", "full"], "default": "full" }
  }
}
```

**Output (success):**
```json
{
  "mode": "rust-only",
  "passed": true,
  "passCount": 5,
  "failCount": 0,
  "skipCount": 1,
  "tailOutput": "..."
}
```

> **v0.1 变更**: 失败时额外返回 `hint` 字段，指导排查。

### 3.5 `codelattice_graph_overview` *(v0.1)*

获取图概览：节点/边/符号/包计数，按 kind 分组，质量和诊断摘要。不含完整 graph，适合 AI 快速评估项目。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "nodeCount": 7,
  "edgeCount": 6,
  "symbolCount": 2,
  "packageCount": 2,
  "sourceFileCount": 1,
  "nodeKindCounts": { "symbol": 2, "package": 1, "source-file": 1, "repository": 1, "target": 1, "diagnostic": 1 },
  "edgeKindCounts": { "CALLS": 1, "DEFINES": 2, "CONTAINS_PACKAGE": 1, "HAS_TARGET": 1, "OWNS_SOURCE": 1 },
  "qualitySummary": { "total": 7, "passed": 7, "failed": 0 },
  "diagnosticsSummary": { "total": 1, "bySeverity": { "info": 1 } }
}
```

### 3.6 `codelattice_unresolved_report` *(v0.1)*

报告未解析调用和诊断。Rust: 展示低 confidence 或 unresolved reason 的 CALLS 边，按 reason 分组。Cangjie: 返回 supported=false。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100 }
  },
  "required": ["root"]
}
```

**Output (Rust, success):**
```json
{
  "language": "rust",
  "supported": true,
  "total": 3,
  "unresolvedEdges": 2,
  "unresolvedDiagnostics": 1,
  "reasonBreakdown": { "call-cross-crate-unresolved": 2 },
  "topItems": [ { "source": "...", "target": "...", "confidence": 0.3, "reason": "call-cross-crate-unresolved", "callKind": "method" } ],
  "diagnosticItems": [ { "code": "use-path-unresolved", "message": "...", "severity": "info", "path": "src/main.rs" } ],
  "stopLineNote": "Items near Rust stop-line..."
}
```

**Output (Cangjie):**
```json
{
  "language": "cangjie",
  "supported": false,
  "reason": "Cangjie does not track unresolved calls in v0.1",
  "total": 0,
  "items": []
}
```

### 3.7 `codelattice_symbol_search` *(v0.1)*

按名称搜索符号（大小写不敏感子串匹配）。可选按 kind 过滤。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "query": { "type": "string", "description": "搜索查询（大小写不敏感子串匹配）" },
    "kind": { "type": "string", "description": "按符号类型过滤（function, struct, class 等）" },
    "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 100 }
  },
  "required": ["root", "query"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "query": "helper",
  "matchCount": 1,
  "matches": [ { "id": "symbol:c1-same-module::crate::helper", "name": "helper", "kind": "function", "file": "src/lib.rs", "line": 1, "label": "symbol" } ]
}
```

### 3.8 `codelattice_export_bridge` *(v0.1)*

导出 GitNexus-RC bridge JSON 到 /tmp。仅导出，不做 Tool import。输出路径必须位于 /tmp。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python"], "description": "必须显式指定语言" },
    "outputPath": { "type": "string", "description": "输出文件路径（必须在 /tmp 下）。默认自动生成" }
  },
  "required": ["root", "language"]
}
```

**Output (success):**
```json
{
  "outputPath": "/tmp/codelattice-bridge-1715328000000.json",
  "bytes": 12345,
  "schemaVersion": "0.3.0",
  "language": "rust",
  "packages": 2,
  "files": 1,
  "symbols": 3,
  "relationships": 6,
  "stdoutPurity": true
}
```

---

### 3.9 `codelattice_symbol_context` *(v0.2, enhanced v0.4, v0.11)*

获取符号的丰富上下文：定义位置、**源码片段**、出边/入边（按 kind 分组）、相关诊断、confidence 样本、**关联文档**。若匹配多个符号则返回候选列表。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "name": { "type": "string", "description": "符号名称" },
    "kind": { "type": "string", "description": "按符号类型过滤" },
    "limit": { "type": "integer", "default": 10, "maximum": 50 },
    "includeSnippet": { "type": "boolean", "default": true, "description": "是否包含源码片段（v0.4 新增）" },
    "snippetContext": { "type": "integer", "default": 3, "maximum": 10, "description": "源码片段前后上下文行数（v0.4 新增）" }
  },
  "required": ["root", "name"]
}
```

**Output (success):**
```json
{
  "query": "helper",
  "ambiguous": false,
  "selected": true,
  "matchCount": 1,
  "candidates": [
    {
      "id": "symbol:c1-same-module::crate::helper",
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
      "outgoingEdges": { "CALLS": 0 },
      "incomingEdges": { "CALLS": 1, "DEFINES": 1 },
      "relatedDiagnostics": 0,
      "confidenceSamples": [ { "confidence": 0.9, "reason": "call-same-module-resolved" } ]
    }
  ],
  "relatedDocs": [
    { "path": "docs/guide.md", "section": "Helper Functions", "confidence": 0.8, "matchType": "inline-code" }
  ],
  "note": "Single match selected automatically"
}
```

> **v0.4 变更**: 每个候选新增 `sourceSnippet` 对象（默认开启，可通过 `includeSnippet: false` 关闭）。文件不存在或读取失败时返回 `{ "warning": "...", "lines": null }` 而非 panic。片段上限 50 行，上下文默认 3 行（最大 10）。

> **v0.11 变更**: 新增 `relatedDocs` 数组字段。基于静态文档扫描（markdown 文件），返回与查询符号相关的文档条目（最多 5 条）。每条包含 `path`（repo 相对路径）、`section`（标题段落）、`confidence`（0-1）、`matchType`（`inline-code` / `markdown-link` / `code-fence` / `heading-section`）。无匹配时返回空数组。

### 3.10 `codelattice_calls_from` *(v0.2)*

追踪符号的出边调用链（BFS）。返回调用树，每条边带 confidence/reason。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "symbol": { "type": "string", "description": "源符号名称" },
    "depth": { "type": "integer", "default": 1, "minimum": 1, "maximum": 3 },
    "limit": { "type": "integer", "default": 20, "maximum": 100 }
  },
  "required": ["root", "symbol"]
}
```

**Output (success):**
```json
{
  "symbol": "main_fn",
  "direction": "outgoing",
  "depth": 1,
  "callCount": 1,
  "calls": [
    { "target": "helper", "targetId": "symbol:c1-same-module::crate::helper", "edgeKind": "CALLS", "confidence": 0.9, "reason": "call-same-module-resolved" }
  ]
}
```

### 3.11 `codelattice_calls_to` *(v0.2)*

追踪符号的入边调用者（反向 BFS）。了解谁依赖该符号。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "symbol": { "type": "string", "description": "目标符号名称" },
    "depth": { "type": "integer", "default": 1, "minimum": 1, "maximum": 3 },
    "limit": { "type": "integer", "default": 20, "maximum": 100 }
  },
  "required": ["root", "symbol"]
}
```

**Output (success):**
```json
{
  "symbol": "helper",
  "direction": "incoming",
  "depth": 1,
  "callerCount": 1,
  "callers": [
    { "source": "main_fn", "sourceId": "symbol:c1-same-module::crate::main_fn", "edgeKind": "CALLS", "confidence": 0.9, "reason": "call-same-module-resolved" }
  ]
}
```

### 3.12 `codelattice_impact_preview` *(v0.2, enhanced v0.10, v0.11)*

预览符号变更的影响范围：受影响的节点/边、风险等级、风险原因、影响指标、置信度摘要、审查焦点、**可能需要更新的文档**。只读，不写。

> **注意：** risk 是 graph-based preview，不是编译器级完整证明。low-confidence / unknown hunk 是安全信号，不是失败。riskReasons 是给 AI 安排 review focus 用的。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
    "symbol": { "type": "string", "description": "要分析影响范围的符号名称" },
    "direction": { "type": "string", "enum": ["upstream", "downstream", "both"], "default": "both" },
    "depth": { "type": "integer", "default": 2, "minimum": 1, "maximum": 3 },
    "limit": { "type": "integer", "default": 50, "maximum": 200 },
    "compact": { "type": "boolean", "default": false, "description": "Compact 模式：保留 risk/riskReasons/impactMetrics/confidenceSummary/reviewFocus，impactedSymbols 只保留 id/name/kind/file/line，不返回 snippet" }
  },
  "required": ["root", "symbol"]
}
```

**Output (success):**
```json
{
  "symbol": "helper",
  "targetId": "sym_helper_1",
  "direction": "both",
  "risk": "LOW",
  "reasons": ["Small blast radius, few callers"],
  "impactedNodeCount": 2,
  "impactedSymbols": [ { "id": "...", "name": "helper", "kind": "function", "file": "src/lib.rs", "line": 1 } ],
  "impactedNodesByKind": { "function": 2 },
  "impactedEdgesByKind": { "CALLS": 1 },
  "topImpactedFiles": [ { "file": "src/lib.rs", "impactedNodeCount": 2 } ],
  "riskReasons": ["Small blast radius, few callers"],
  "impactMetrics": {
    "callerCount": 1,
    "downstreamCount": 1,
    "impactedFileCount": 1,
    "crossFileCount": 0,
    "publicSymbolCount": 2,
    "testFileCount": 0,
    "lowConfidenceEdgeCount": 0,
    "mediumConfidenceEdgeCount": 0,
    "highConfidenceEdgeCount": 1,
    "unknownConfidenceEdgeCount": 0,
    "totalEdgesConsidered": 1
  },
  "confidenceSummary": {
    "totalEdgesConsidered": 1,
    "highConfidenceCount": 1,
    "mediumConfidenceCount": 0,
    "lowConfidenceCount": 0,
    "unknownConfidenceCount": 0,
    "minConfidence": "1.00",
    "avgConfidence": "1.00",
    "maxConfidence": "1.00"
  },
  "reviewFocus": {
    "topCallers": [],
    "topCallees": [],
    "topFiles": [{ "file": "src/lib.rs", "impactedNodeCount": 2 }],
    "lowConfidenceEdges": [],
    "publicSymbols": [],
    "testFiles": []
  },
  "relatedDocs": [
    { "path": "docs/api.md", "section": "Helper", "confidence": 0.7, "matchType": "inline-code" }
  ],
  "docsLikelyNeedUpdate": [
    { "path": "docs/api.md", "section": "Helper", "reason": "references symbol in inline-code", "confidence": 0.7 }
  ],
  "previewOnly": true,
  "noWrites": true
}
```

### 3.13 `codelattice_query_graph` *(v0.2)*

参数化图查询：按 nodeKind/edgeKind/nameContains/fileContains 过滤。不接受任意查询字符串。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "nodeKind": { "type": "string", "description": "按节点类型过滤（function, struct, class, package 等）" },
    "edgeKind": { "type": "string", "description": "按边类型过滤（CALLS, DEFINES, IMPORTS 等）" },
    "nameContains": { "type": "string", "description": "按名称过滤（大小写不敏感子串）" },
    "fileContains": { "type": "string", "description": "按文件路径过滤（大小写不敏感子串）" },
    "limit": { "type": "integer", "default": 50, "maximum": 200 }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "nodeCount": 2,
  "edgeCount": 0,
  "nodes": [ { "id": "...", "name": "helper", "kind": "function", "file": "src/lib.rs" } ],
  "matchedEdges": [],
  "truncated": false
}
```

### 3.14 `codelattice_project_overview` *(v0.2, enhanced v0.11)*

项目综合概览：身份、统计、top kinds、质量、诊断、hotspots（高扇出）、dense files、**文档关联摘要**。适合打开项目时首次调用。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "nodeCount": 7,
  "symbolCount": 2,
  "sourceFileCount": 1,
  "packageCount": 2,
  "topNodeKinds": [ { "kind": "symbol", "count": 2 }, { "kind": "package", "count": 1 } ],
  "qualitySummary": { "total": 7, "passed": 7, "failed": 0 },
  "qualityMetrics": {
    "graphCompleteness": { "nodeCount": 7, "edgeCount": 6, "symbolCount": 2, "sourceFileCount": 1, "danglingEdgeCount": 0 },
    "edgeConfidence": { "totalConfidenceEdgeCount": 4, "highConfidenceEdgeCount": 3, "mediumConfidenceEdgeCount": 1, "lowConfidenceEdgeCount": 0, "unknownConfidenceEdgeCount": 2, "lowConfidenceEdgeRate": 0.0, "unknownConfidenceEdgeRate": 0.33 },
    "callQuality": { "callEdgeCount": 2, "highConfidenceCallEdgeCount": 2, "mediumConfidenceCallEdgeCount": 0, "lowConfidenceCallEdgeCount": 0, "unknownConfidenceCallEdgeCount": 0, "lowConfidenceCallRate": 0.0 },
    "dependencyQuality": { "importEdgeCount": 1, "includeEdgeCount": 0, "unresolvedImportOrIncludeCount": 0 },
    "diagnostics": { "diagnosticCount": 1, "unresolvedDiagnosticCount": 0, "parseDiagnosticCount": 0 },
    "generatedFrom": { "graphBased": true, "compilerVerified": false, "heuristic": true }
  },
  "diagnosticsSummary": { "total": 1, "bySeverity": { "info": 1 } },
  "hotspots": [],
  "denseFiles": [],
  "docs": {
    "docCount": 12,
    "docSectionCount": 45,
    "docLinkCount": 30,
    "docSymbolReferenceCount": 18,
    "docCodeFenceCount": 8,
    "topDocPaths": ["docs/architecture/mcp-v0-contract.md", "README.md"]
  }
}
```

> **v0.11 变更**: 新增 `docs` 对象字段。基于静态 markdown 扫描，提供文档统计摘要。无 markdown 文件时返回空对象 `{}`。
>
> **v0.14 变更**: 新增 `qualityMetrics` 对象字段。跨语言统一质量指标，包含图完整性、边置信度分布、调用质量、依赖质量和诊断分类。`project_overview`（compact 和 full 模式）、`project_insights`、`review_plan`（release_check 模式）和 `production_assist` 均返回此字段。

### 3.15 `codelattice_repo_registry` *(v0.2)*

列出已知 repo 或检查当前 root 状态。CodeLattice 不维护持久化 registry，每次调用重新分析。完整 registry 管理请使用 GitNexus-RC Tool。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "action": { "type": "string", "enum": ["list", "status"], "default": "status" },
    "root": { "type": "string", "description": "项目根路径（status action 必填）" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" }
  }
}
```

**Output (status, success):**
```json
{
  "action": "status",
  "root": "/path/to/project",
  "indexed": true,
  "nodeCount": 7,
  "symbolCount": 2,
  "language": "rust",
  "note": "CodeLattice does not maintain a persistent registry. Each call analyzes fresh."
}
```

### 3.16 `codelattice_rename_preview` *(v0.2)*

预览重命名操作：查找定义、引用边、受影响文件。只读，不做 AST 安全校验。返回 `applySupported: false`。实际重命名请使用 IDE/language server。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "symbol": { "type": "string", "description": "当前符号名称" },
    "newName": { "type": "string", "description": "建议的新名称" },
    "kind": { "type": "string", "description": "符号类型（用于消歧）" }
  },
  "required": ["root", "symbol", "newName"]
}
```

**Output (success):**
```json
{
  "symbol": "helper",
  "newName": "assist",
  "applySupported": false,
  "candidates": [
    { "id": "...", "name": "helper", "kind": "function", "file": "src/lib.rs", "line": 1, "referenceCount": 1, "affectedFiles": ["src/lib.rs"] }
  ],
  "note": "Preview only. Use IDE/language server for safe AST-aware renames."
}
```

### 3.17 `codelattice_cache_status` *(v0.3, enhanced v0.13)*

查询进程内分析缓存的状态，包含内存层和持久化层。可选按 root/language 过滤。不触发分析。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "可选：按根路径过滤（substring match）" },
    "language": { "type": "string", "description": "可选：按语言过滤" }
  }
}
```

**Output (success, nested two-layer format):**
```json
{
  "memory": {
    "entryCount": 2,
    "maxEntries": 16,
    "entries": [
      {
        "root": "/path/to/project",
        "language": "rust",
        "strict": false,
        "cacheKey": "/path/to/project:rust:false",
        "layer": "memory",
        "createdAtMs": 5230,
        "lastUsedAtMs": 120,
        "hitCount": 3,
        "analysisDurationMs": 58,
        "trackedFiles": 12
      }
    ],
    "totalHits": 5,
    "totalMisses": 2,
    "totalEvictions": 0,
    "persistentHits": 1,
    "persistentMisses": 0
  },
  "persistent": {
    "enabled": true,
    "cacheDir": "/path/to/cache-dir",
    "entryCount": 1,
    "totalSizeBytes": 24576,
    "entries": [
      {
        "root": "/path/to/project",
        "language": "rust",
        "createdAt": "2026-05-15T12:00:00",
        "analysisDurationMs": 58,
        "trackedFiles": 12,
        "sizeBytes": 24576
      }
    ]
  }
}
```

持久化层未启用时：
```json
{
  "memory": { "entryCount": 0, "maxEntries": 16, "entries": [], "totalHits": 0, "totalMisses": 0, "totalEvictions": 0, "persistentHits": 0, "persistentMisses": 0 },
  "persistent": {
    "enabled": false,
    "reason": "CODELATTICE_CACHE=off or directory unavailable"
  }
}
```

### 3.18 `codelattice_cache_clear` *(v0.3, enhanced v0.13)*

清空分析缓存，支持选择清除内存层、持久化层或两者。可选按 root/language 过滤。不影响 Tool registry 或源文件。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "可选：只清除指定根路径的缓存" },
    "language": { "type": "string", "description": "可选：只清除指定语言的缓存" },
    "layer": { "type": "string", "enum": ["memory", "persistent", "both"], "default": "memory", "description": "要清除的缓存层。使用 'persistent' 或 'both' 同时清除磁盘缓存。" }
  }
}
```

**Output (success):**
```json
{
  "clearedCount": 2,
  "remainingCount": 0,
  "layer": "both"
}
```

---

> **v0.13 Cache Signal**: All v0.2+ tools (3.9–3.16) include `cacheHit` (boolean) and `analysisDurationMs` (u64, only on miss) in their output JSON. First call for a given root+language+strict is always a cache miss; subsequent calls return `cacheHit: true` without re-running the analyze subprocess. The `codelattice_analyze` (3.1) tool also includes these signals. **v0.13 adds persistent cache**: when `CODELATTICE_CACHE_DIR` is set, analysis results survive process restarts. The lookup order is memory → persistent → fresh analyze. The `codelattice_cache_status` tool returns nested `{memory, persistent}` status. The `codelattice_cache_clear` tool accepts a `layer` parameter ("memory"/"persistent"/"both").

### 3.19 `codelattice_production_assist` *(enhanced v0.11)*

Dry-run production readiness assistant。Aggregates quality gates、unresolved calls、diagnostics、changed symbol impact、**文档更新建议**。Read-only。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "changedSymbols": { "type": "array", "items": { "type": "string" }, "description": "Optional list of symbol names you changed" }
  },
  "required": ["root"]
}
```

**Output**: `{ symbolCount, nodeCount, edgeCount, qualityGatesPassed, qualityGatesFailed, unresolvedCalls, diagnostics, risk, topFiles, changedSymbols (with sourceSnippet), recommendations, docsLikelyNeedUpdate, docAssociationSummary, dryRun: true }`

> **v0.11 变更**: 新增 `docsLikelyNeedUpdate` 数组（基于 changedSymbols 关联的文档）和 `docAssociationSummary` 对象（文档扫描统计）。无 changedSymbols 时 `docsLikelyNeedUpdate` 为空数组。

### 3.20 `codelattice_compare_runs`

Compare two analysis results: nodes/edges/symbols/quality gates/diagnostics diff。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Compare cached vs fresh (if no bridge files)" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "beforeBridgeJson": { "type": "string", "description": "Path to 'before' bridge JSON file" },
    "afterBridgeJson": { "type": "string", "description": "Path to 'after' bridge JSON file" }
  }
}
```

**Output**: `{ beforeNodes, afterNodes, nodeDelta, addedNodes, removedNodes, beforeEdges, afterEdges, edgeDelta, addedEdges, removedEdges, beforeSymbols, afterSymbols, symbolDelta, beforeDiagnostics, afterDiagnostics, summary, note }`

---

### 3.21 `codelattice_cache_prewarm`

Pre-warm the process-local analysis cache for a project. Runs analysis and stores the result so subsequent tool calls hit cache immediately. If cache is already fresh (mtime-valid), returns cacheHit=true immediately.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Project root directory (absolute path)" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "python", "auto"], "default": "auto" },
    "strict": { "type": "boolean", "default": false, "description": "Strict mode. Default false to match most other tools." }
  },
  "required": ["root"]
}
```

**Output**: `{ warmed: bool, cacheHit: bool, analysisDurationMs, summary: { symbolCount, nodeCount, edgeCount, sourceFileCount } }`

**Use case**: AI agent opens a project → calls cache_prewarm → all subsequent graph queries return instantly from cache.

---

### 3.22 `codelattice_changed_symbols`

Detect changed symbols from git diff. Maps diff hunks to graph symbols using sourcePath + lineStart/lineEnd overlap detection. Read-only, no writes.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Project root (absolute path, must be a git repo)" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "auto"], "default": "auto" },
    "diffMode": { "type": "string", "enum": ["working-tree", "staged", "unstaged", "head"], "default": "working-tree" },
    "baseRef": { "type": "string", "description": "Optional git ref to compare against" },
    "compact": { "type": "boolean", "default": true },
    "includeSnippet": { "type": "boolean", "default": true },
    "snippetContext": { "type": "integer", "default": 2, "minimum": 0, "maximum": 10 },
    "limit": { "type": "integer", "default": 100, "maximum": 500 }
  },
  "required": ["root"]
}
```

**Output**:
```json
{
  "changedFiles": [{ "path": "...", "changeKind": "modified", "hunkCount": 2 }],
  "changedSymbols": [{ "id": "...", "name": "...", "kind": "...", "file": "...", "line": 10, "lineEnd": 20, "changeKinds": ["modified"], "hunkCount": 1, "callerCount": 5, "risk": "MEDIUM" }],
  "unknownHunks": [{ "file": "...", "hunkStart": 1, "hunkEnd": 3, "hunkLines": 3, "reason": "hunk does not overlap with any known symbol" }],
  "deletedFiles": [{ "path": "..." }],
  "renamedFiles": [{ "path": "..." }],
  "summary": { "changedFileCount": 2, "changedSymbolCount": 3, "unknownHunkCount": 1, "deletedFileCount": 0, "renamedFileCount": 0 },
  "diffMode": "working-tree",
  "previewOnly": true,
  "noWrites": true
}
```

**Mapping strategy**: For each diff hunk, checks if the hunk line range [new_start, new_start + new_count - 1] overlaps with any symbol's [startLine, endLine]. Unknown hunks occur when: changes are outside any symbol range, new files have no graph symbols yet, or deleted files can't be mapped.

**Error handling**: Non-git repos return a graceful error. Git diff failures don't panic. Unknown hunks are a normal safety output, not a failure.

---

### 3.23 `codelattice_production_assist` (updated with auto-detect + risk summary, enhanced v0.10)

When `changedSymbols` is not provided, automatically runs `git diff` to detect changed symbols and includes:
- `autoDetectedChangedSymbols: true/false`
- `changedSymbolCount`
- `changedSymbols` array (with id/name/kind/file/line/risk/callerCount)
- `unknownHunkCount`
- `unknownHunks`
- `changedFileCount`

**v0.10 Enhanced Risk Summary** (new fields):
- `overallRisk` — aggregated risk level (LOW/MEDIUM/HIGH) from changed symbols + project health
- `overallRiskReasons` — array of human-readable reasons for the overall risk
- `changedSymbolImpacts` — per-symbol risk breakdown with callerCount, lowConfidenceEdges, reasons
- `highestRiskSymbols` — top 5 symbols sorted by caller count (most dangerous first)
- `reviewChecklist` — actionable items for AI agents:
  - "inspect direct callers of each changed symbol via codelattice_symbol_context"
  - "inspect N low-confidence edge(s) — these may be indirect or ambiguous calls"
  - "run focused tests for affected test files identified in impact set"
  - "review N unknown hunk(s) manually — diff region(s) could not be mapped to known symbols"
  - "address N failed quality gate(s) before proceeding"

> **注意：** unknown hunks 不等于失败，但意味着需要人工/AI复核。production_assist 会将 unknown hunks 写入 overallRiskReasons 和 reviewChecklist。

---

### 3.24 `codelattice_dead_code_candidates` *(v0.10)*

Identify static dead-code candidates — symbols and files with no incoming edges or unreachable from detected entry points. Returns candidates with confidence scoring, risk cautions, and verification suggestions. **NOT deletion proof.** Always use `codelattice_impact_preview` and project tests before deleting any code.

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Project root (absolute path)" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "arkts", "typescript", "c", "cpp", "python", "auto"], "default": "auto" },
    "compact": { "type": "boolean", "default": true },
    "limit": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200 },
    "includeFiles": { "type": "boolean", "default": true },
    "includeSymbols": { "type": "boolean", "default": true },
    "includeTests": { "type": "boolean", "default": false },
    "includePublicApi": { "type": "boolean", "default": true },
    "entryHints": { "type": "array", "items": { "type": "string" }, "description": "Symbol names or file path substrings to treat as entry points" },
    "excludePatterns": { "type": "array", "items": { "type": "string" }, "description": "File path patterns to exclude" }
  },
  "required": ["root"]
}
```

**Output:**
- `summary` — candidateSymbolCount, candidateFileCount, high/medium/low confidence counts, publicApiCautionCount, dynamicFeatureCautionCount
- `candidateSymbols` — scored symbol candidates with reasons, cautions, recommendedVerification
- `candidateFiles` — scored file candidates with reasons and cautions
- `entryPoints` — detected entry points used for reachability analysis
- `warnings` — e.g., `entry-point-detection-low-confidence` if no entry points found
- `generatedFrom` — `{ graphBased: true, compilerVerified: false, heuristic: true, deletionSafe: false }`

**Scoring strategy:**
- Symbol candidates: +0.35 no incoming edges, +0.25 unreachable from entry points, +0.15 private visibility, -0.35 public/exported, -0.40 entry-like name, -0.15 dynamic pattern
- File candidates: +0.35 no incoming file edges, +0.20 no entry-like symbols, +0.20 all symbols are candidates, -0.30 contains public exports, -0.40 entry-like filename
- Confidence: high (>=0.80), medium (>=0.55), low (<0.55). Minimum score 0.45 to appear in output.

**Known limitations:**
- Static graph analysis only — no control flow, type inference, or macro expansion
- Dynamic dispatch / reflection / plugin systems may hide callers
- Public/exported APIs may be called by external consumers
- Build configs (cfg, features) may conditionally include code
- Test-only code may be critical for CI

---

### 3.25 `codelattice_impact_analysis` *(v0.12)*

Change impact analysis — find what breaks if a symbol changes.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Project root directory" },
    "language": { "type": "string", "enum": ["rust","cangjie","arkts","typescript","c","cpp","python","auto"] },
    "target": { "type": "string", "description": "Target symbol name" },
    "compact": { "type": "boolean", "default": false },
    "depth": { "type": "integer", "default": 2, "minimum": 1, "maximum": 3 }
  },
  "required": ["root"]
}
```

**Output:**
- `targetMatched` — matched symbol info (name, kind, file, line)
- `directCallers` — symbols that directly call/depend on target
- `indirectCallers` — transitively affected symbols (up to depth)
- `entryPointPaths` — paths from entry points to target (if reachable)
- `riskScore` — 0..1 based on caller count, cross-directory, public API, entry reachability
- `suggestions` — `readFirst` (files to read before changing), `reviewFirst` (files to review after changing)
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

---

### 3.26 `codelattice_risk_hotspots` *(v0.12)*

Project risk hotspot detection — identify high-risk symbols and files.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string" },
    "language": { "type": "string" },
    "compact": { "type": "boolean", "default": false },
    "maxResults": { "type": "integer", "default": 20, "maximum": 100 }
  },
  "required": ["root"]
}
```

**Output:**
- `summary` — totalSymbols, totalFiles, hotspotCount, averageFanOut
- `hotspotSymbols` — symbols ranked by risk (fan-in + fan-out + cross-module + public API signals)
- `hotspotFiles` — files ranked by risk (symbol count + edge density + cross-module)
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

---

### 3.27 `codelattice_architecture_drift` *(v0.12)*

Architecture health check — detect cycles, cross-layer violations, boundary leaks.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string" },
    "language": { "type": "string" },
    "compact": { "type": "boolean", "default": false },
    "layerRules": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "pathPattern": { "type": "string" },
          "mayDependOn": { "type": "array", "items": { "type": "string" } }
        }
      },
      "description": "User-defined layer rules for drift detection"
    }
  },
  "required": ["root"]
}
```

**Output:**
- `summary` — totalNodes, totalEdges, cycleCount, crossLayerViolationCount, avgCoupling
- `cycles` — detected cycle candidates (DFS-based, path + symbols involved)
- `crossLayerCalls` — calls that violate layerRules (if provided)
- `reverseDependencies` — modules that depend "upward" in the layer stack
- `couplingHotspots` — overly coupled modules (high cross-module edge density)
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

---

### 3.28 `codelattice_ai_context_pack` *(v0.12)*

AI editing context — given a task, output the right files and symbols to read.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string" },
    "language": { "type": "string" },
    "task": { "type": "string", "description": "Natural language description of the editing task" },
    "targetSymbols": { "type": "array", "items": { "type": "string" }, "description": "Specific symbol names to include" },
    "compact": { "type": "boolean", "default": false }
  },
  "required": ["root"]
}
```

**Output:**
- `contextFiles` — relevant files ranked by relevance, with reason
- `keySymbols` — symbols to understand (name, kind, file, reason)
- `callChains` — important call paths involving target symbols
- `dependencyNotes` — "depends on X in file Y" notes
- `suggestedReadOrder` — ordered file list for sequential reading
- `usefulCommands` — suggested follow-up MCP calls (e.g., `codelattice_impact_analysis`)
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Note:** This tool performs keyword matching against graph data — no LLM invocation.

---

### 3.29 `codelattice_review_gate` *(v0.12)*

Diff-based review gate — analyze changed files for risk and produce a review checklist.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string" },
    "language": { "type": "string" },
    "useGitDiff": { "type": "boolean", "default": true },
    "changedFiles": { "type": "array", "items": { "type": "string" }, "description": "Explicit file list (used when useGitDiff=false)" },
    "compact": { "type": "boolean", "default": false }
  },
  "required": ["root"]
}
```

**Output:**
- `touchedSymbols` — symbols in changed files (name, kind, file, line)
- `hotspotExposure` — whether any touched symbols are in the risk hotspot set
- `impactSummary` — direct callers/imports of touched symbols
- `reviewChecklist` — ordered items to review (risk-ranked)
- `riskLevel` — `low` | `medium` | `high` | `critical`
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

---

### 3.25 `codelattice_impact_analysis` *(v0.19)*

Change impact analysis: given a target symbol name, find direct/indirect callers and callees, entry point reachability, risk scoring, and actionable recommendations.

**Input:**

```json
{
  "root": { "$ref": "#/definitions/rootPath" },
  "language": { "$ref": "#/definitions/language" },
  "target": { "type": "string", "description": "Symbol name to analyze impact for" },
  "compact": { "type": "boolean", "default": false },
  "depth": { "type": "integer", "default": 2, "minimum": 1, "maximum": 3 }
}
```

**Output:**
- `targetMatched` — matched symbol (name, kind, file, line) or null
- `directCallers` — symbols that directly call the target
- `directCallees` — symbols the target directly calls
- `upstreamPath` — trace to entry points (if reachable)
- `downstreamPath` — trace to leaf symbols
- `riskScore` — 0.0..1.0 based on caller count, cross-directory, public API, entry reachability
- `riskLevel` — "low" / "medium" / "high" / "critical"
- `suggestions` — readFirst, reviewFirst arrays with file/symbol recommendations
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Known limitations:**
- Static graph analysis only — no runtime dispatch resolution
- Dynamic calls / trait objects / reflection may hide actual callers
- Cross-crate callers not visible without workspace-level analysis

---

### 3.26 `codelattice_risk_hotspots` *(v0.19)*

Project-level risk hotspot detection: identify high fan-in/fan-out symbols and files, cross-module dependencies, and public API exposure.

**Input:**

```json
{
  "root": { "$ref": "#/definitions/rootPath" },
  "language": { "$ref": "#/definitions/language" },
  "compact": { "type": "boolean", "default": false },
  "maxResults": { "type": "integer", "default": 20, "maximum": 100 }
}
```

**Output:**
- `summary` — totalSymbols, totalFiles, hotspotSymbolCount, hotspotFileCount, avgFanIn, avgFanOut, maxFanIn, maxFanOut
- `hotspotSymbols` — symbols with high fan-in/fan-out, scored by cross-module + entry/public signals
- `hotspotFiles` — files with high dependency concentration
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Scoring:**
- Fan-in/fan-out ratio normalized to 0..1
- Cross-directory bonus +0.2, public API +0.15, entry reachable +0.1
- Threshold: symbols in top 20% by combined score appear as hotspots

---

### 3.27 `codelattice_architecture_drift` *(v0.19)*

Architecture health analysis: detect cycle candidates, cross-layer calls (with optional user-provided layer rules), boundary leaks, and overly coupled modules.

**Input:**

```json
{
  "root": { "$ref": "#/definitions/rootPath" },
  "language": { "$ref": "#/definitions/language" },
  "compact": { "type": "boolean", "default": false },
  "layerRules": {
    "type": "array",
    "description": "Optional layer rules: each rule specifies layer name and allowed downstream layers",
    "items": {
      "type": "object",
      "properties": {
        "layer": { "type": "string" },
        "allowedDeps": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

**Output:**
- `summary` — totalSymbols, totalFiles, cycleCount, crossLayerViolationCount, boundaryLeakCount, coupledModuleCount
- `cycles` — detected cycle candidates (each cycle: list of symbol names + file paths forming the loop)
- `crossLayerViolations` — calls violating user-provided layer rules (if layerRules given)
- `boundaryLeaks` — symbols with unexpectedly wide dependency span
- `coupledModules` — directory pairs with bidirectional dependencies
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Cycle detection:**
- DFS-based cycle detection on CALLS/IMPORTS edges (max depth 20)
- Reports cycles as ordered symbol chains
- Without `layerRules`, reports structural cycles only; with `layerRules`, additionally checks layer conformance

---

### 3.28 `codelattice_ai_context_pack` *(v0.19)*

AI editing context: given a task description or target symbols, output relevant files, key symbols, call chains, dependency notes, and suggested read order — ready to feed into AI assistants. No LLM invocation.

**Input:**

```json
{
  "root": { "$ref": "#/definitions/rootPath" },
  "language": { "$ref": "#/definitions/language" },
  "task": { "type": "string", "description": "Natural language description of the editing task" },
  "targetSymbols": { "type": "array", "items": { "type": "string" }, "description": "Optional symbol names to focus on" },
  "compact": { "type": "boolean", "default": false }
}
```

**Output:**
- `contextFiles` — relevant files with relevance score, reason, and snippet of key symbols
- `keySymbols` — symbols most relevant to the task, with callers/callees summary
- `callChains` — important dependency paths connecting key symbols
- `dependencyNotes` — "this file depends on X", "changing Y will affect Z" style notes
- `suggestedReadOrder` — ordered list of files for AI to read first
- `usefulCommands` — suggested follow-up MCP tool calls (e.g., impact_preview, calls_to)
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Matching:**
- Keyword matching against symbol names, file paths, and graph metadata
- Task description tokens matched to symbol/file names (case-insensitive substring)
- Graph BFS from matched symbols to find connected context

---

### 3.29 `codelattice_review_gate` *(v0.19)*

Diff-based review gate: analyze git diff or specified changed files → touched symbols → hotspot exposure → impact summary → review checklist → risk level.

**Input:**

```json
{
  "root": { "$ref": "#/definitions/rootPath" },
  "language": { "$ref": "#/definitions/language" },
  "useGitDiff": { "type": "boolean", "default": true },
  "changedFiles": { "type": "array", "items": { "type": "string" }, "description": "Explicit file list (used when useGitDiff=false)" },
  "compact": { "type": "boolean", "default": false }
}
```

**Output:**
- `touchedSymbols` — symbols in changed files
- `hotspotExposure` — touched symbols that are also risk hotspots
- `impactSummary` — upstream/downstream impact of touched symbols
- `reviewChecklist` — actionable review items generated from the analysis
- `riskLevel` — "low" / "medium" / "high" / "critical"
- `riskReasons` — human-readable explanations for the risk level
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Risk level determination:**
- "low": ≤3 touched symbols, none are hotspots, no cross-module impact
- "medium": 4-9 touched symbols, or any hotspot touched
- "high": ≥10 touched symbols, or critical infrastructure symbols touched
- "critical": entry points or public API symbols among touched symbols with significant downstream impact

---

### 3.30 `codelattice_reachability_map` *(v0.20)*

Compute reachability map from detected entry points. Returns entry points, reachable symbols, and unreachable candidates with confidence scores.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "Project root directory (absolute path)" },
    "language": { "type": "string", "enum": ["rust","cangjie","arkts","typescript","c","cpp","python","auto"], "default": "auto" },
    "maxDepth": { "type": "integer", "description": "Max traversal depth (default 8)", "default": 8, "minimum": 1, "maximum": 32 },
    "entryHints": { "type": "array", "items": { "type": "string" }, "description": "Optional entry point hints" },
    "includeUnreachable": { "type": "boolean", "description": "Include unreachable candidates (default true)", "default": true },
    "compact": { "type": "boolean", "description": "Compact mode", "default": false },
    "includeReachableItems": { "type": "boolean", "description": "Include reachable symbol/file lists (default false)", "default": false },
    "excludePatterns": { "type": "array", "items": { "type": "string" }, "description": "Glob patterns to exclude from unreachable candidates" },
    "limit": { "type": "integer", "description": "Max items per category (default 50, max 200)", "default": 50, "maximum": 200 }
  },
  "required": ["root"]
}
```

**Output:**
- `language` — detected language
- `summary` — `{ entryPointCount, reachableSymbolCount, unreachableCandidateCount, totalSymbols, reachableFileCount, totalFiles }`
- `entryPoints` — detected entry points (id, name, kind, file, line, confidence, score, reasons)
- `reachable` — `{ symbolCount, fileCount }` (or full symbols/files if `includeReachableItems`)
- `unreachableCandidates` — symbols not reachable from any entry point (with cautions)
- `warnings` — always includes "static graph reachability only" and "dynamic dispatch may hide runtime reachability"
- `generatedFrom` — `{ staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`

**Entry point detection heuristics (per language):**
- Rust: main, lib.rs public functions, test functions
- TypeScript/ArkTS: index.ts exports, main/server/app entry files
- Python: \_\_init\_\_.py, \_\_main\_\_.py, setup.py, wsgi/asgi modules
- C/C++: main, WinMain, DllMain
- Cangjie: main package entries

**Known limitations:**
- Static graph analysis only — no runtime dispatch resolution
- Dynamic calls / trait objects / reflection may hide actual callers
- Cross-crate callers not visible without workspace-level analysis
- Not a deletion safety guarantee

---

### 3.22 Cangjie Symbol Search Fix (v0.6)

> **v0.6 Fix**: Cangjie graph nodes use `kind="symbol"` with display name in `label` field, while Rust uses `kind="function"/"method"/...` with `label="symbol"`. The old `symbol_search` filtered by `label == "symbol"`, which excluded all Cangjie symbols.
>
> **Fix**: Filter by `kind` (symbol, function, method, class, etc.) instead of `label`. Name extraction now tries `properties.name` → `label` (Cangjie) → `id` parsing (both `::` and `:` separators). File extraction handles Cangjie `id` format `sym:<file>:<Kind>:<name>#<arity>`.

---

### 3.23 Profile Detection (v0.7)

The `initialize` response now includes profile information:

```json
{
  "serverInfo": {
    "name": "codelattice",
    "version": "0.7.0",
    "cangjieSupport": true,
    "arktsSupport": true,
    "typescriptSupport": true,
    "toolCount": 31
  }
}
```

- `cangjieSupport`: `true` if binary compiled with `--features tree-sitter-cangjie`, `false` otherwise
- `arktsSupport`: `true` if binary compiled with `--features tree-sitter-arkts`, `false` otherwise
- `typescriptSupport`: `true` if binary compiled with `--features tree-sitter-typescript`, `false` otherwise
- `toolCount`: number of tools exposed via `tools/list`

Scripts parse this output to detect the binary's capabilities and warn if optional language support is missing.

---

### 3.x Cache Evolution (v0.5)

> **v0.5 Cache Enhancements**:
> - **mtime-based invalidation**: Cache entries track source file mtimes. On next call, if any file was added/removed/modified, cache automatically invalidates and re-analyzes.
> - **LRU eviction**: Max 16 cache entries. When over limit, least-recently-used entry is evicted.
> - **Enhanced cache_status**: Now includes `maxEntries`, `totalEvictions`, `cacheKey`, `trackedFiles`.
> - **cache_clear** supports filtering by root/language (unchanged from v0.3).

### 3.x Snippet Expansion (v0.5)

> **v0.5 Source Snippet Expansion**:
> - `calls_from` / `calls_to`: source candidates and edges now include `sourceSnippet`/`targetSnippet` (controlled by `includeSnippet` param, default true).
> - `impact_preview`: now includes `impactedSymbols` array with snippets and `contextSnippet` in top files.
> - `query_graph`: matched nodes can include `sourceSnippet` (controlled by `includeSnippet`, default false for compact output).
> - `rename_preview`: candidates now include `sourceSnippet` by default.

---

## 四、错误格式

所有错误通过 MCP `isError: true` 返回：

```json
{
  "content": [{ "type": "text", "text": "{\"error\": \"<code>\", \"message\": \"<human-readable>\", \"details\": \"<optional>\", \"hint\": \"<optional>\"}" }],
  "isError": true
}
```

> **v0.1 变更**: 错误结构新增 `details` 和 `hint` 字段。

### 4.1 错误码

| Code | When |
|------|------|
| `path_denied` | Root path is on the deny list (live repo) |
| `path_not_found` | Root path does not exist |
| `path_not_directory` | Root path is not a directory |
| `output_path_denied` *(v0.1)* | Export output path not under /tmp |
| `command_failed` | Subprocess exited non-zero |
| `timeout` | Subprocess exceeded time limit |
| `json_parse_failed` | Subprocess output not valid JSON |
| `json_serialize_failed` *(v0.1)* | Failed to serialize bridge JSON |
| `write_failed` *(v0.1)* | Failed to write export file |
| `missing_parameter` *(v0.1)* | Required parameter missing |
| `cangjie_disabled` | Cangjie requested but feature not compiled |
| `cpp_disabled` | C++ requested but feature not compiled |
| `smoke_failed` | Smoke script reported failure |
| `unknown_tool` | Tool name not recognized |

---

## 五、Safety Rules

1. **No live repo writes** — MCP v0.1 只读项目源码，不修改任何文件
2. **Temp files only** — export_bridge 仅写入 /tmp，路径校验拒绝非 /tmp 路径
3. **No default switch** — MCP server 不修改任何默认工具配置
4. **No generatedAt strict comparison** — generatedAt 不参与 deterministic compare
5. **Path deny list** — `/Users/jiangxuanyang/Desktop/cangjie` 等生产 live repo 默认拒绝
6. **Timeout protection** — 所有 subprocess 有超时保护
7. **Stdout purity** — stdout 只输出 JSON-RPC，不混入其他文本

---

## 六、Example MCP Session

```
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"codelattice","version":"0.1.0"}}}

→ {"jsonrpc":"2.0","method":"notifications/initialized"}

→ {"jsonrpc":"2.0","id":2,"method":"tools/list"}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"codelattice_analyze","description":"...","inputSchema":{...}},...]}}

→ {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codelattice_summary","arguments":{"root":"/Users/jiangxuanyang/Desktop/codelattice","language":"rust"}}}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"language\":\"rust\",\"root\":\"...\",\"graphSummary\":{...},\"qualitySummary\":{...}}"}]}}
```

---

## 七、Implementation Location

- **Source**: `crates/cli/src/mcp_server.rs`
- **Integration**: `crates/cli/src/main.rs` — `Mcp` subcommand
- **Tests**: `crates/cli/tests/mcp_server.rs`
- **Start command**: `gitnexus-rust-core-cli mcp`
- **Sidecar wrapper**: `scripts/codelattice-mcp.sh` — AI client 启动入口（详见 `docs/architecture/mcp-local-client-setup.md`）
- **Client config**: `docs/architecture/mcp-local-client-setup.md`

---

## 八、Known Limitations (v0.3)

- No streaming / partial results
- No graph persistence between MCP server restarts (cache is process-local only)
- No multi-project awareness (single root per call)
- Cangjie requires `--features tree-sitter-cangjie` compile flag
- Smoke test paths are workspace-relative (not portable across machines)
- `symbol_search` / `find_symbols` uses simple substring match, no fuzzy/search index
- `unresolved_report` for Cangjie returns supported=false (no CALLS confidence classification)
- `export_bridge` output restricted to /tmp only
- `rename_preview` does not perform AST-safe rewrite — use IDE/language server for actual renames
- `query_graph` only matches edges if `edgeKind` parameter is provided; node-only queries return empty `matchedEdges`

### C++ Phase A (v0.13)

C++ 支持 (`.cpp`/`.hpp`/`.cc`/`.cxx`/`.h`) 已进入 Phase A 阶段。可通过 `--language cpp` 和 `language: "cpp"` MCP 参数使用。

**支持范围：**
- namespace、class、struct、method、function、constructor、destructor 符号提取
- enum / enum class、using alias、typedef 符号提取
- macro（`#define`）识别
- `#include` 依赖提取
- 函数调用识别（带 confidence tier：same-file > cross-file heuristic）
- `codelattice_analyze`、`codelattice_project_overview`、`codelattice_symbol_search`、`codelattice_symbol_context`、`codelattice_query_graph`、`codelattice_impact_preview`、`codelattice_changed_symbols`、`codelattice_production_assist` 全部可用
- CLI `--format json` 和 `--format gitnexus-rc` (bridge) 输出

**C++ 特性未启用行为：**
- 若 binary 未编译 `--features tree-sitter-cpp`，调用 C++ 相关工具返回 `cpp_disabled` 错误（类似 Cangjie 的 `cangjie_disabled`）

**已知限制（C++）：**
- 不做完整预处理（No full preprocessing）
- 不执行构建系统（No build system execution）
- 不依赖 compile_commands.json（No compile_commands.json requirement）
- 不做模板实例化（No template instantiation）
- 不做完整重载解析（No full overload resolution）
- 不做虚函数派发解析（No virtual dispatch resolution）
- 不是 clangd 的替代（Not a replacement for clangd）
- 编译需要 `--features tree-sitter-cpp`

---

### TypeScript Phase A (v0.12)

TypeScript 支持 (`.ts`/`.tsx`) 已进入 Alpha / production trial 阶段。可通过 `--language typescript` 和 `language: "typescript"` MCP 参数使用。

**支持范围：**
- 函数、类、方法、接口、类型别名、变量、枚举的符号提取
- 命名/默认/命名空间 import 识别
- 函数调用、类型引用、成员访问、new 表达式的基础识别
- `codelattice_analyze`、`codelattice_project_overview`、`codelattice_symbol_search`、`codelattice_symbol_context`、`codelattice_query_graph`、`codelattice_impact_preview`、`codelattice_changed_symbols`、`codelattice_production_assist` 全部可用
- CLI `--format json` 和 `--format gitnexus-rc` (bridge) 输出
- TSX 文件通过 tree-sitter TSX 语法解析

**已知限制（TypeScript）：**
- 不运行 npm/tsc — 无类型检查，无模块解析
- 不替代 tsserver/IDE
- 不解析 node_modules、path alias、monorepo workspace 引用
- 方法调用置信度较低（无法确定调用目标类型）
- 不支持 JSX 框架语义（React component detection 等）
- 编译需要 `--features tree-sitter-typescript`
- `impact_preview` risk heuristic is simple (node/edge count thresholds), not context-aware
- `repo_registry` does not maintain persistent state — defers to GitNexus-RC Tool for full registry
- BFS traversal for calls_from/calls_to/impact_preview limited to max depth 3
- Cache key does NOT include `strict` for v0.2 tools (always false), but analyze defaults strict=true — cross-tool cache reuse only between tools with same strict value

---

## 九、变更历史

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-10 | v0.1.0 | 初始版本 — 4 tools, stdio JSON-RPC, subprocess approach |
| 2026-05-10 | v0.1.0+1 | v0.1 — 4 new tools (graph_overview, unresolved_report, symbol_search, export_bridge), output shaping, unified error structure, dogfood harness |
| 2026-05-10 | v0.2.0 | v0.2 — 8 new tools (symbol_context, calls_from, calls_to, impact_preview, query_graph, project_overview, repo_registry, rename_preview), shared GraphView layer, BFS traversal, read-only graph intelligence |
| 2026-05-11 | v0.3.0 | v0.3 — 2 new tools (cache_status, cache_clear), process-local analysis cache, cacheHit/analysisDurationMs signals in all tool outputs, 18 tools total |
| 2026-05-11 | v0.4.0 | v0.4 — source snippets in symbol_context (includeSnippet, snippetContext, sourceSnippet field), install-mcp.sh, wrapper --self-test, cache smoke script, real client readiness |
| 2026-05-11 | v0.5.0 | v0.5 — Daily-use candidate: mtime-based cache invalidation, LRU eviction (max 16), snippet expansion to calls_from/to/impact/query/rename, 2 new tools (production_assist, compare_runs), install --doctor, real-client-dry-run.sh, 20 tools total |
| 2026-05-11 | v0.6.0 | v0.6 — opencode real client verified, cangjie symbol_search fix (kind-based filtering, id parsing, label fallback), pipe-buffer deadlock fix, path-deny false positive fix, 1 new tool (cache_prewarm), 21 tools total |
| 2026-05-11 | v0.7.0 | v0.7 — Install/profile hardening: cangjieSupport in initialize serverInfo, wrapper binary selection prefers cangjie-enabled binaries, install-mcp.sh --build defaults with cangjie feature, --rust-only option, doctor checks cangjie support + cangjie smoke, cargo run fallback includes tree-sitter-cangjie, 21 tools total |
| 2026-05-15 | v0.13.0 | v0.13 beta profile — initialize serverInfo includes Cangjie/ArkTS/TypeScript/C/C++/Python support flags, release packaging builds all optional language adapters, release smoke verifies seven portable language fixtures, 24 tools total |
| 2026-05-11 | v0.8.0 | v0.8 — Cangjie Live Production Runway: live repo deny-list exemption for runtime/cjgui subpath (ALLOWED_DENIED_SUBPATHS), cangjie-live-codelattice-smoke.sh (--dry-run/--analyze/--mcp/--tool-ingest/--full), Tool registry entry cangjie-live-codelattice (17,194 nodes / 52,522 edges / 2,887 symbols), explicit naming convention (cangjie-live-codelattice vs cjgui-index vs legacy cjgui), 21 tools total |
| 2026-05-13 | v0.9.0 | v0.9 — Changed-Symbol Auto Detection: 1 new tool (codelattice_changed_symbols), git diff → graph symbol mapping via hunk overlap detection, production_assist auto-detects changed symbols when changedSymbols not provided, 8 new integration tests (temp git repo fixture), 22 tools total |
| 2026-05-13 | v0.10.0 | v0.10 — Better Impact Risk Reasons: impact_preview enhanced with riskReasons (human-readable risk explanations), impactMetrics (callerCount/downstreamCount/impactedFileCount/crossFileCount/publicSymbolCount/testFileCount/confidence edge counts), confidenceSummary (min/avg/max confidence), reviewFocus (topCallers/topCallees/topFiles/lowConfidenceEdges/publicSymbols/testFiles), compact mode. production_assist enhanced with overallRisk/overallRiskReasons/changedSymbolImpacts/highestRiskSymbols/reviewChecklist. unknown hunks surface in risk reasons and checklist. 10 new integration tests, 76 total, 22 tools total |
