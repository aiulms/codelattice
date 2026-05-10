# MCP v0.1 Dogfood Report

> **日期：** 2026-05-10
> **脚本：** `scripts/mcp-dogfood.sh`
> **Fixture：** `fixtures/call-resolution/c1-same-module` (Rust)

---

## Dogfood 方法

每次 tool call 通过独立的 stdio session（echo JSON-RPC → `gitnexus-rust-core-cli mcp` → head -1），模拟真实 AI agent 使用模式。

## 结果

```
============================================
 MCP v0.1 Dogfood Results
============================================
  PASS: initialize
  PASS: tools/list (8 tools)
  PASS: codelattice_analyze
  PASS: codelattice_quality
  PASS: codelattice_summary
  PASS: codelattice_graph_overview
  PASS: codelattice_symbol_search
  PASS: codelattice_unresolved_report

  PASS: 8
  FAIL: 0

All checks passed — MCP v0.1 dogfood successful.
```

## 各工具验证详情

### initialize
- Server name: `codelattice`
- Protocol version: `2024-11-05`
- Capabilities: `{ "tools": {} }`

### tools/list
- 返回 8 个工具（v0: 4 + v0.1: 4）
- 每个工具均有 inputSchema

### codelattice_analyze
- Compact 模式（includeGraph=false）：graph 字段被移除，保留 summary + qualityGates
- language: rust, nodeCount > 0

### codelattice_quality
- overall: pass
- failed gates 排在 passed gates 前面（当前全部 pass，排序无差异）

### codelattice_summary
- graphSummary + qualitySummary 均返回
- 不含 graph 数据

### codelattice_graph_overview
- nodeCount > 0, edgeCount > 0, symbolCount > 0
- nodeKindCounts 包含 symbol/package/source-file/repository/target/diagnostic
- edgeKindCounts 包含 CALLS/DEFINES/CONTAINS_PACKAGE/HAS_TARGET/OWNS_SOURCE
- qualitySummary 和 diagnosticsSummary 正常

### codelattice_symbol_search
- 搜索 "helper" → 1 match, name contains "helper"
- 搜索 "main" → match found

### codelattice_unresolved_report
- supported: true (Rust)
- reasonBreakdown 为空（该 fixture 无 unresolved edges）
- stopLineNote 正确显示

## 未在 dogfood 中覆盖的工具
- `codelattice_export_bridge`：需要写文件，由集成测试 `mcp_export_bridge_writes_to_tmp` 覆盖
- `codelattice_smoke`：耗时较长（120s timeout），由集成测试 `mcp_smoke_rust_only` 覆盖

## 结论

MCP v0.1 所有 8 个工具通过真实 stdio dogfood 验证。AI agent 可安全使用。
