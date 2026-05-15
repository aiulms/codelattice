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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" }
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" }
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" }
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp"], "description": "必须显式指定语言" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" }
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

### 3.15 `codelattice_repo_registry` *(v0.2)*

列出已知 repo 或检查当前 root 状态。CodeLattice 不维护持久化 registry，每次调用重新分析。完整 registry 管理请使用 GitNexus-RC Tool。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "action": { "type": "string", "enum": ["list", "status"], "default": "status" },
    "root": { "type": "string", "description": "项目根路径（status action 必填）" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" }
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "cpp", "auto"], "default": "auto" },
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
    "toolCount": 21
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
| 2026-05-15 | v0.13.0 | v0.13 beta.2 — initialize serverInfo adds arktsSupport/typescriptSupport, release packaging builds all optional language adapters, release smoke verifies Rust/Cangjie/ArkTS/TypeScript portable fixtures, 22 tools total |
| 2026-05-11 | v0.8.0 | v0.8 — Cangjie Live Production Runway: live repo deny-list exemption for runtime/cjgui subpath (ALLOWED_DENIED_SUBPATHS), cangjie-live-codelattice-smoke.sh (--dry-run/--analyze/--mcp/--tool-ingest/--full), Tool registry entry cangjie-live-codelattice (17,194 nodes / 52,522 edges / 2,887 symbols), explicit naming convention (cangjie-live-codelattice vs cjgui-index vs legacy cjgui), 21 tools total |
| 2026-05-13 | v0.9.0 | v0.9 — Changed-Symbol Auto Detection: 1 new tool (codelattice_changed_symbols), git diff → graph symbol mapping via hunk overlap detection, production_assist auto-detects changed symbols when changedSymbols not provided, 8 new integration tests (temp git repo fixture), 22 tools total |
| 2026-05-13 | v0.10.0 | v0.10 — Better Impact Risk Reasons: impact_preview enhanced with riskReasons (human-readable risk explanations), impactMetrics (callerCount/downstreamCount/impactedFileCount/crossFileCount/publicSymbolCount/testFileCount/confidence edge counts), confidenceSummary (min/avg/max confidence), reviewFocus (topCallers/topCallees/topFiles/lowConfidenceEdges/publicSymbols/testFiles), compact mode. production_assist enhanced with overallRisk/overallRiskReasons/changedSymbolImpacts/highestRiskSymbols/reviewChecklist. unknown hunks surface in risk reasons and checklist. 10 new integration tests, 76 total, 22 tools total |
