# CodeLattice MCP Tool Surface Consolidation — Preflight & Execution Card

> Status: **EXECUTION** (self-analyzed, Oracle-pending)
> Date: 2026-05-19
> Author: Sisyphus (lead)

## 0. Self-Analysis（替代 Metis）

Metis 超时（26min未完成），我自行完成风险分析。

### 0.1 核心设计决策

**Q: Facade 如何调用底层工具？**

方案：Facade handler 直接调用底层 handler 函数，然后用 `unwrap_tool_result()` 提取内层 JSON。
- 不修改任何现有 handler 代码
- 不提取 compute 函数（避免触碰现有代码）
- 仅新增一个 helper：`fn unwrap_tool_result(result: &Value) -> Value`

```rust
fn unwrap_tool_result(result: &Value) -> Value {
    result["content"][0]["text"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}))
}
```

**Q: Mode 参数冲突？**

不存在。CodeLattice 的参数命名已经统一（root, language, compact, limit, includeTests 等）。Facade 只是透传 params 给子 handler。

**Q: compact 参数链？**

两层独立：
- Facade 的 `compact` 控制 facade envelope（nextActions 剪辑、result 精简）
- 底层工具的 `compact` 控制底层输出（由工具自行解释）
- 单 mode 时：facade 透传 compact 给子工具 + facade 自身按 compact 处理 envelope
- multi-tool mode（full/full_review）时：子工具不开 compact（获取完整数据），facade 层做统一切削

**Q: multi-tool mode 失败处理？**

`full` / `full_review` / `safe_cleanup_review` 模式调用多个子工具：
- 全部成功 → 合并输出
- 部分失败 → 返回 partial results + 错误列表
- 全部失败 → 返回错误

**Q: 工具数膨胀（42 → 50）？**

缓解措施：
- `CODELATTICE_MCP_TOOLSET=core` 只暴露 ~16 个（8 facade + 8 standalone）
- 默认 `full` 保持兼容
- 50 个工具对 AI 客户端是可处理的（Claude Desktop/opencode 已验证）

**Q: underlyingTools 字段语义？**

运行时填充：列出本次调用实际使用的底层工具。
- mode="overview" → `["codelattice_analyze", "codelattice_project_overview"]`
- mode="search" → `["codelattice_symbol_search"]`
- mode="full" → `["codelattice_analyze", "codelattice_quality", "codelattice_project_overview", "codelattice_project_insights"]`

**Q: 哪些工具不做 facade 包装？**

以下 8 个保持 standalone（即使在 core mode）：
- `codelattice_smoke` — 开发/调试
- `codelattice_repo_registry` — 仓库生命周期管理
- `codelattice_rename_preview` — 一次性 edge case
- `codelattice_compare_runs` — 对比分析（edge case）
- `codelattice_export_bridge` — 导出（非分析）
- `codelattice_automation_graph` — 专项扫描
- `codelattice_ai_context_pack` — AI 编辑专用
- `codelattice_cache_prewarm` — 缓存预热

以下 2 个不暴露（无论 core/full）：无（全部保留兼容）

### 0.2 识别风险点

| # | 风险 | 等级 | 缓解 |
|---|---|---|---|
| R1 | 50 工具让 AI 选择困难 | MEDIUM | core mode 只暴露 ~16，tools/list 加 category + recommended 标记 |
| R2 | facade handler 中调用多个子工具，`&mut McpCache` 借出问题 | LOW | 串行调用，Rust borrow checker 允许 |
| R3 | unwrap_tool_result 解析失败 | LOW | fallback 到 `json!({})`，不 panic |
| R4 | 已有测试中的工具计数断言 | MEDIUM | 更新 38→42 的测试（之前做过）+ 新增 facade 测试无数字断言 |
| R5 | tools/list 响应体积增大（+category 字段） | LOW | ~2KB 增量，可忽略 |
| R6 | core mode 下 AI 调了被隐藏的工具 | LOW | 返回友好错误 "use CODELATTICE_MCP_TOOLSET=full" |
| R7 | facade mode invalid 时错误信息不够友好 | LOW | 返回 validModes 列表 |

## 1. 设计方案

### 1.1 新增 8 个 Facade Tools

| Facade Tool | Modes | 覆盖的底层工具 | 参数需求 |
|---|---|---|---|
| `codelattice_project` | overview, quality, insights, full | analyze, quality, summary, project_overview, project_insights | root 必填, language 可选 |
| `codelattice_symbol` | search, context, callers, callees, graph | symbol_search, symbol_context, calls_from, calls_to, query_graph | root 必填, name/query 按 mode 不同 |
| `codelattice_change_review` | changed_symbols, impact, production_assist, breaking_change, consistency, full_review | changed_symbols, impact_preview, production_assist, breaking_change_review, consistency_review | root 必填, target/symbol 按 mode 不同 |
| `codelattice_cleanup` | dead_code, reachability, external_api, framework_entries, safe_cleanup_review | dead_code_candidates, reachability_map, external_api_surface, framework_entry_hints | root 必填, entryHints 可选 |
| `codelattice_workspace` | graph, impact, overview, full | workspace_graph, cross_project_impact | root 必填, target 仅 impact mode 需要 |
| `codelattice_release_check` | quick, full, config, docs_tests, breaking_changes | quality, review_gate, config_examples_review, breaking_change_review, consistency_review | root 必填 |
| `codelattice_cache` | status, clear, explain | cache_status, cache_clear | 无必填参数 |
| `codelattice_workflow` | onboarding, before_edit, after_edit, delete_code, release_check, legacy_cleanup | workflow_presets, review_plan | scenario/mode 决定参数, language 可选 |

### 1.2 统一 Facade 输出格式

```json
{
  "schemaVersion": "facade.v1",
  "tool": "codelattice_project",
  "mode": "overview",
  "language": "rust",
  "root": "/path",
  "summary": {"riskLevel": "low", "keyFinding": "3 projects, 142 files"},
  "result": { /* mode-specific */ },
  "nextActions": ["Use codelattice_symbol search to explore symbols"],
  "cautions": ["static analysis only", "no runtime proof"],
  "generatedFrom": {"staticAnalysis": true, "runtimeVerified": false, "scriptsExecuted": false},
  "compact": false,
  "underlyingTools": ["codelattice_project_overview"]
}
```

compact=true 时：
- 保留 identity（tool, mode, language, root）
- 保留 summary.riskLevel
- 保留 nextActions
- 保留 cautions, generatedFrom
- 保留 underlyingTools
- **省略** result 中的大数组（nodes[], edges[], affectedProjects[], paths[]）
- **省略** summary 中的详细数字（只留 riskLevel）

### 1.3 Toolset 分层

```
CODELATTICE_MCP_TOOLSET=core → 暴露 facade tools + 8 standalone
CODELATTICE_MCP_TOOLSET=full → 暴露全部（默认）
```

实现位置（3 处）：
1. `tools_list()` → 在返回前过滤 array
2. `tools/call` dispatch → core mode 下拒绝隐藏工具
3. `initialize` → 在 serverInfo 中暴露 toolset mode

### 1.4 Tool 元数据扩展

每个工具定义新增（仅 full mode 时输出）：
```json
{
  "name": "codelattice_project",
  "category": "facade",
  "stability": "stable",
  "recommended": true
}
```

分类体系：
- `facade` — 8 个 facade tools
- `analyze` — analyze, quality, summary, project_overview, project_insights, graph_overview, unresolved_report
- `symbol` — symbol_search, symbol_context, calls_from, calls_to, query_graph
- `change` — changed_symbols, impact_preview, impact_analysis, production_assist, breaking_change_review, consistency_review, review_gate
- `cleanup` — dead_code_candidates, reachability_map, external_api_surface, framework_entry_hints
- `workspace` — workspace_graph, cross_project_impact
- `release` — config_examples_review, review_plan, risk_hotspots, architecture_drift, ai_context_pack
- `cache` — cache_status, cache_clear, cache_prewarm
- `util` — smoke, repo_registry, rename_preview, compare_runs, export_bridge, automation_graph, workflow_presets

## 2. 实现清单

### 2.1 新增代码

| # | 内容 | 位置 | 行数 |
|---|---|---|---|
| 1 | `unwrap_tool_result()` helper | mcp_server.rs, near tool_result() | 8 |
| 2 | `wrap_facade_output()` helper | mcp_server.rs, new section | 40 |
| 3 | `validate_facade_mode()` helper | mcp_server.rs, new section | 15 |
| 4 | `filter_tools_by_toolset()` helper | mcp_server.rs, near tools_list() | 30 |
| 5 | 8 facade 工具定义 | mcp_server.rs, tools_list() end | 200 |
| 6 | 8 facade handler 函数 | mcp_server.rs, before run_mcp_server | 500 |
| 7 | 8 dispatch 条目 | mcp_server.rs, handle_request match | 16 |
| 8 | toolset 过滤逻辑 | mcp_server.rs, 3 insertion points | 15 |
| 9 | 元数据字段（category/recommended/stability） | mcp_server.rs, tools_list() per-tool | 84 |
| 10 | 版本注释更新 | mcp_server.rs, line 4 | 2 |
| 11 | Facade 测试 | crates/cli/tests/mcp_server.rs | 200 |
| 12 | Facade smoke 脚本 | scripts/ | 180 |
| 13 | Doc 更新 | README/CHANGELOG/docs | 40 |

**总估算：~1330 行新增**

### 2.2 不修改

- 不删除任何现有 handler 函数
- 不改变任何现有工具定义
- 不修改 handler 签名
- 不改变 tool_result() 行为
- 不修改 McpCache 结构

## 3. 执行顺序

1. 新增 helper 函数（unwrap_tool_result, wrap_facade_output, validate_facade_mode, filter_tools_by_toolset）
2. 新增 8 个 facade handler 函数
3. 新增 8 个工具定义到 tools_list()
4. 新增 8 个 dispatch 条目
5. 添加 toolset 过滤逻辑（3 处）
6. 添加元数据字段到 tools_list()（category/recommended/stability）
7. 更新版本注释
8. `cargo check -p gitnexus-rust-core-cli`
9. 新增 facade 测试到 mcp_server.rs
10. 创建 facade smoke 脚本
11. 全量验证（cargo test, fmt, smoke, regression）
12. Doc 更新
13. Commit + push

## 4. 验证矩阵

| # | 验证 | 命令 | 预期 |
|---|---|---|---|
| V1 | cargo check | `cargo check` | 0 errors |
| V2 | cargo test | `cargo test` | All pass |
| V3 | cargo fmt | `cargo fmt --check` | Clean |
| V4 | git diff | `git diff --check` | Clean |
| V5 | MCP self-test | `bash scripts/codelattice-mcp.sh --self-test` | All pass |
| V6 | Dogfood | `bash scripts/mcp-dogfood.sh` | All pass（注意更新工具数检查） |
| V7 | Workspace smoke | `bash scripts/codelattice-mcp-workspace-smoke.sh` | 14/14 pass |
| V8 | Facade smoke | `bash scripts/codelattice-mcp-facade-smoke.sh` | All pass |
| V9 | Full toolset test | `CODELATTICE_MCP_TOOLSET=full bash scripts/...` | 50 tools |
| V10 | Core toolset test | `CODELATTICE_MCP_TOOLSET=core bash scripts/...` | ~16 tools |
| V11 | GitNexus | `node .../gitnexus/dist/cli/index.js detect-changes` | No unexpected changes |

## 5. Stop-lines

- 不删除任何 handler / 工具定义
- 不修改现有 MCP schema 中的 required 字段
- 不修改 GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool
- 不执行目标项目代码
- 不新增外部依赖

## 6. File Map

| 文件 | 操作 | 关键行号 |
|---|---|---|
| `crates/cli/src/mcp_server.rs` | 大量新增 | helper: L179, tools_list: L7732-8430, dispatch: L12209-12282, handlers: before L16187 |
| `crates/cli/tests/mcp_server.rs` | 新增测试 | 现有测试末尾 |
| `scripts/codelattice-mcp-facade-smoke.sh` | 新建 | |
| `README.md` | 编辑 | 工具数说明 |
| `CHANGELOG.md` | 编辑 | [Unreleased] 段 |
| `docs/architecture/mcp-tool-surface.md` | 新建 | |
