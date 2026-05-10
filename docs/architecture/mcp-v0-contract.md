# MCP v0 Contract — CodeLattice Thin stdio Wrapper

> **日期：** 2026-05-10
> **版本：** v0.1.0
> **状态：** Active (MCP v0)
> **定位：** AI agent 可通过 MCP JSON-RPC 调用 CodeLattice CLI 的分析/质量/概要/Smoke 能力

---

## 一、定位

MCP v0 是 CodeLattice CLI 的 thin stdio wrapper：

- **Read-only** — 只读项目分析，不写源码
- **Rust/Cangjie only** — 仅支持 Rust 和 Cangjie 两种语言
- **Not GitNexus-RC replacement** — 不替代 GitNexus-RC MCP server
- **Not default tool switch** — 显式 opt-in
- **No persistence** — 不做 graph 存储、repo 注册、embeddings
- **No impact/cypher** — 不做 impact analysis、Cypher 查询

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
    "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto" },
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
    "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto" }
  },
  "required": ["root"]
}
```

**Output (success):**
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "allPassed": true,
  "exitCode": 0,
  "gates": [ { "gateName": "duplicate_nodes", "passed": true, "detail": "..." } ]
}
```

### 3.3 `codelattice_summary`

返回紧凑的 graph stats + quality summary（不含完整 graph）。

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": { "type": "string", "description": "项目根目录绝对路径" },
    "language": { "type": "string", "enum": ["rust", "cangjie", "auto"], "default": "auto" }
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
  "qualitySummary": { "allPassed": true, "totalGates": 7, "passedGates": 7, "failedGates": 0 }
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

---

## 四、错误格式

所有错误通过 MCP `isError: true` 返回：

```json
{
  "content": [{ "type": "text", "text": "{\"error\": \"<code>\", \"message\": \"<human-readable>\"}" }],
  "isError": true
}
```

### 4.1 错误码

| Code | When |
|------|------|
| `path_denied` | Root path is on the deny list (live repo) |
| `path_not_found` | Root path does not exist |
| `path_not_directory` | Root path is not a directory |
| `command_failed` | Subprocess exited non-zero |
| `timeout` | Subprocess exceeded time limit |
| `json_parse_failed` | Subprocess output not valid JSON |
| `unsupported_language` | Language not rust/cangjie/auto |
| `cangjie_disabled` | Cangjie requested but feature not compiled |
| `smoke_failed` | Smoke script reported failure |
| `unknown_tool` | Tool name not recognized |

---

## 五、Safety Rules

1. **No live repo writes** — MCP v0 只读项目源码，不修改任何文件
2. **Temp files only** — 临时输出（如 bridge JSON）用后即删
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

---

## 八、Known Limitations (v0)

- No streaming / partial results
- No graph persistence between calls
- No symbol lookup / search
- No impact analysis
- No Cypher/query support
- No multi-project awareness
- Cangjie requires `--features tree-sitter-cangjie` compile flag
- Smoke test paths are workspace-relative (not portable across machines)

---

## 九、变更历史

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-10 | v0.1.0 | 初始版本 — 4 tools, stdio JSON-RPC, subprocess approach |
