# MCP v0.1 Practical AI Layer — Preflight

> **日期：** 2026-05-10
> **前置：** MCP v0 (commit `7a8bc70`)
> **目标：** 在 MCP v0 基础上增加 AI-friendly 查询工具、dogfood 验证、输出整形

---

## 背景

MCP v0 已实现 4 个基础工具（analyze/quality/summary/smoke），通过 10 个集成测试。但 AI agent 实际使用中发现：
1. `codelattice_analyze` 默认返回完整 graph（可能 1MB+），AI context 窗口压力大
2. 缺少轻量级图概览工具（node/edge/symbol counts without full graph）
3. 缺少符号搜索能力
4. 缺少 unresolved calls 报告
5. 缺少 bridge JSON 导出（方便 AI 获取下游消费格式）
6. 错误结构不够 AI-readable（缺少 details/hint）

## 变更范围

### 新增 4 个工具
| Tool | 输入 | 输出 | 用途 |
|------|------|------|------|
| `codelattice_graph_overview` | root, language? | nodeCount/edgeCount/symbolCount + kind breakdowns + quality/diagnostics summary | AI 快速评估图规模 |
| `codelattice_unresolved_report` | root, language?, limit? | unresolved edges + diagnostics grouped by reason, stop-line note | 定位分析盲区 |
| `codelattice_symbol_search` | root, language?, query, kind?, limit? | matching symbols (name/kind/file/line) | 查找符号 |
| `codelattice_export_bridge` | root, language, outputPath? | file path + byte count + schema stats | 导出 bridge JSON 到 /tmp |

### 输出整形（v0 tools 改善）
- `codelattice_analyze`: 默认 compact（去掉 graph），includeGraph=true 时包含
- `codelattice_quality`: failed gates 排在 passed gates 前面
- `codelattice_smoke`: 失败时增加 hint 字段
- 统一错误结构：code / message / details / hint

### Dogfood
- 新增 `scripts/mcp-dogfood.sh`：真实 stdio JSON-RPC 调用 8 个工具

## 硬边界
- 不新增依赖
- 不切默认工具
- 不修改 GitNexus-RC runtime/schema/WebUI
- 不修改 Tool dist
- export_bridge 只写 /tmp
- Cangjie unresolved_report 返回 supported=false（不伪造数据）
- 不做 graph persistence / impact / cypher / embeddings / SSE

## 验证计划
- 18 个 MCP 集成测试全部通过
- dogfood 8/8 通过
- bridge_roundtrip 13/13 通过
- productization_commands 11/11 通过
- alpha smoke 5/5 pass
- cargo fmt --check clean
- git diff --check clean
