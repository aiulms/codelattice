# MCP Local Client Integration Smoke Report

> **日期：** 2026-05-10
> **版本：** v0.2.0
> **状态：** Pass

---

## 一、Wrapper 启动

**脚本路径**: `scripts/codelattice-mcp.sh`

| 检查项 | 结果 |
|--------|------|
| `bash -n` 语法验证 | ✅ Pass |
| `--help` 输出 | ✅ 正确显示用法说明 |
| `--version` 输出 | ✅ 显示 0.2.0、root、binary 路径 |
| 自动选择 debug binary | ✅ 自动选择 `target/debug/gitnexus-rust-core-cli` |
| 从任意 cwd 启动 | ✅ 通过 `SCRIPT_DIR` 自动定位 |
| 环境变量覆盖 | ✅ CODELATTICE_ROOT / CODELATTICE_MCP_BIN 支持 |
| stderr 日志 / stdout 纯净 | ✅ 日志走 stderr，stdout 为 JSON-RPC |

**结论**: Wrapper 稳定可用，AI 客户端可直接使用。

---

## 二、Local Client Smoke 调用工具

| # | 工具 | 结果 | 备注 |
|---|------|------|------|
| 1 | initialize | ✅ PASS | server name = "codelattice" |
| 2 | tools/list | ✅ PASS | 16 tools |
| 3 | codelattice_project_overview | ✅ PASS | fixture, nodeCount=7, symbolCount=2 |
| 4 | codelattice_symbol_context | ✅ PASS | helper 找到, matchCount=1 |
| 5 | codelattice_calls_from | ✅ PASS | main_fn → helper edge |
| 6 | codelattice_impact_preview | ✅ PASS | risk=LOW, 3 impacted nodes |
| 7 | codelattice_rename_preview | ✅ PASS | applySupported=false |
| 8 | project_overview (self) | ⏭ SKIP | 全量自分析 ~90s, 超出 smoke 限制 |

---

## 三、工具体验评估

### 3.1 `codelattice_project_overview`

**输出质量**: 优秀。包含 language、nodeCount、symbolCount、sourceFileCount、packageCount、topNodeKinds、topEdgeKinds、qualitySummary、diagnosticsSummary、hotspots、denseFiles。

**长度评估**: 适中（~500 chars for small fixture, ~1KB for larger projects）。AI 客户端可一次消费。

**改进建议**: 可选加 `topSymbols`（高扇出符号 top-5），帮助 AI 快速了解项目核心。

### 3.2 `codelattice_symbol_context`

**输出质量**: 优秀。返回 candidates 数组（含 id、name、kind、file、line、lineEnd、visibility、outgoingEdges、incomingEdges、relatedDiagnostics、confidenceSamples）。

**ambiguity 表达**: 清楚 — `ambiguous` 布尔 + `matchCount` + `candidates` 数组。单匹配时自动 selected，多匹配时列出所有候选。

**长度评估**: 适中（每个 candidate ~300 chars）。多匹配时可能膨胀，但 `limit` 参数可控。

**改进建议**:
- 可选加 `sourceSnippet`（前后 3 行源码），让 AI 不需要额外读取文件
- `selected` 字段与 `candidates[0]` 信息重复，可考虑简化

### 3.3 `codelattice_calls_from`

**输出质量**: 良好。返回 sourceCandidates + edges（含 depth、confidence、reason、type）。

**长度评估**: 适中。depth=1 时通常 <1KB。

**改进建议**: 
- 可选加 `tree` 格式（嵌套 JSON），目前是扁平 edge 列表，AI 需要自己构建树
- 可加 `callKinds` 统计（method vs function vs constructor）

### 3.4 `codelattice_impact_preview`

**输出质量**: 优秀。返回 risk 等级（LOW/MEDIUM/HIGH）、impactedNodeCount、impactedNodesByKind、impactedEdgesByKind、topImpactedFiles、reasons、previewOnly、noWrites。

**长度评估**: 适中（~500 chars）。适合 AI 快速判断。

**改进建议**:
- 可选加 `diffPreview`（影响的行号范围），而非仅文件级
- risk 等级描述可更具体（如 "LOW: 3 nodes, 1 file affected"）

### 3.5 `codelattice_rename_preview`

**输出质量**: 优秀。返回 candidates（含 confidence、file、filesNeedingReview、incomingCallCount、outgoingCallCount）、applySupported=false、note、warnings。

**长度评估**: 适中（~400 chars per candidate）。

**改进建议**:
- 可加 `estimatedEdits`（预计编辑点数量），帮助 AI 评估复杂度

---

## 四、Path Guard 验证

Live cangjie repo deny list 在 MCP server 中硬编码，wrapper 不做额外检查（正确）。所有测试使用 fixture 或 CodeLattice 自身，未触发 deny list。

---

## 五、Dogfood 验证

`scripts/mcp-dogfood.sh` 结果：17/17 PASS
- initialize ✅
- tools/list (16 tools) ✅
- codelattice_analyze ✅
- codelattice_quality ✅
- codelattice_summary ✅
- codelattice_graph_overview ✅
- codelattice_symbol_search ✅
- codelattice_unresolved_report ✅
- codelattice_export_bridge ✅
- codelattice_symbol_context ✅
- codelattice_calls_from ✅
- codelattice_calls_to ✅
- codelattice_impact_preview ✅
- codelattice_query_graph ✅
- codelattice_project_overview ✅
- codelattice_repo_registry ✅
- codelattice_rename_preview ✅

---

## 六、v0.3 优先级建议

按 AI 客户端消费体验排序：

| 优先级 | 改进 | 理由 |
|--------|------|------|
| P0 | **Local cache** — GraphView 进程内缓存，避免每次 tool call 重新分析 | 当前每次调用 3-8s（fixture）/ 60-90s（全项目），对 AI 交互不可接受 |
| P1 | **Source snippet** — symbol_context 返回前后 3 行源码 | AI 不需额外读文件即可理解上下文 |
| P2 | **Prebuilt binary install** — `scripts/install.sh` 安装到 ~/.local/bin | 避免 AI 客户端依赖 cargo 和源码 checkout |
| P3 | **production_assist_dry_run** — 模拟 GitNexus Tool 导入前检查 | 与 GitNexus 工作流衔接 |
| P4 | **compare_runs** — 两次分析结果差异 | 追踪项目演变 |
| P5 | **Tree format calls** — calls_from/calls_to 嵌套格式 | 减少 AI 解析负担 |

---

## 七、是否建议接入真实本机 AI 客户端

**建议：条件性接入**。

条件：
1. **必须先解决 local cache（P0）** — 当前每次调用 3-90s 的延迟对交互体验不可接受。需要实现进程内 GraphView 缓存（首次分析后复用，直到 root 变更或显式 refresh）。
2. **建议提供 prebuilt binary（P2）** — 让 AI 客户端配置更简单，不依赖 cargo。
3. **MCP client 需支持 stdio transport** — Codex / Claude Desktop / opencode 均支持。

满足条件 1 后即可接入。当前可用于低频场景（项目首次分析、质量检查），但不适合高频交互（逐符号查询）。

---

## 八、文件清单

| 文件 | 类型 |
|------|------|
| `scripts/codelattice-mcp.sh` | MCP startup wrapper |
| `docs/architecture/mcp-local-client-setup.md` | Client config examples |
| `scripts/mcp-local-client-smoke.sh` | Local client integration smoke |
| `docs/plans/2026-05-10-mcp-local-client-integration-smoke-report.md` | This report |
