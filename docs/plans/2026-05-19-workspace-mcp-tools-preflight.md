# Preflight: Workspace Graph & Cross-Project Impact MCP Tools

**日期**: 2026-05-19
**状态**: Pre-flight
**作者**: Sisyphus (AI)

## 目标

将 WebUI Runner 已有的 Workspace Graph 和 Cross-Project Impact 能力正式暴露为 MCP 工具，让 AI 客户端可以不打开 WebUI、不依赖 Runner 状态，直接通过 MCP 分析 workspace 跨项目关系和影响。

## 交付物

1. `codelattice_workspace_graph` MCP 工具
2. `codelattice_cross_project_impact` MCP 工具
3. Workspace fixture (含 Makefile + Dockerfile)
4. MCP workspace smoke test
5. 文档更新 (README, CHANGELOG, mcp-v0-contract.md)

## 现有能力

### WebUI Runner (Python, scripts/webui-runner.py)

- `_workspace_graph_build()`: 从 workspace root 扫描子项目，解析 manifest，构建跨项目关系图
- `_ws_impact_resolve_target()`: 7 级目标解析
- `_ws_impact_analyze()`: BFS 遍历，风险评估
- `_ws_insights_graph_summary()`: 图谱摘要

### Rust MCP Server (crates/cli/src/mcp_server.rs)

- 38 个 MCP 工具
- `compute_automation_graph()`: 文件扫描模式（无缓存依赖）
- `handle_impact_analysis()`: 分析缓存模式
- `handle_project_insights()`: 洞察模式

## Schema 对齐

- 输出 schema 与 WebUI Runner 概念一致：
  - `workspace.graph.v1` / `workspace.impact.v1`
  - Node kinds: workspace, project, config, script, workflow, unsupported
  - Edge kinds: contains, depends_on, imports, script_refs, config_refs, adjacent_to, unsupported_boundary
  - Confidence 策略一致
  - Caution 声明一致

## 设计约束

- 不依赖 WebUI Runner 进程
- 不要求 workspaceRunId
- 不依赖 .codelattice-webui 目录
- 所有输出标注 static-only / heuristic
- 不允许 dangling edge
- 扫描上限：depth=5, entries=5000

## 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| mcp_server.rs 过大 | 高 | 中 | 考虑新 crate 或模块拆分 |
| manifest 解析复杂 | 中 | 中 | 复用 project-model 已有能力 |
| fixture 不稳定 | 低 | 中 | 确保 Makefile + Dockerfile 都存在 |
| 工具总数超预期 | 低 | 低 | 只加 2 个，38→40 |

## Execution Card

### Write Set
- `crates/cli/src/mcp_server.rs` (新增 2 个工具注册 + 处理函数)
- `fixtures/workspace/` (新增 workspace fixture)
- `scripts/codelattice-mcp-workspace-smoke.sh` (新增 smoke test)
- `CHANGELOG.md` (新增 Unreleased 条目)
- `README.md` (简要说明新工具)
- `docs/architecture/mcp-v0-contract.md` (新增工具契约)
- `docs/plans/` (本文档 + closure doc)

### Forbidden Set
- 不修改 GitNexus-RC / Tool / CodeLattice-Tool
- 不修改 WebUI Runner 代码
- 不修改 AI client 配置
- 不修改真实项目源码
- 不引入外部依赖

### Stop-line
- 不做完整的构建系统解析
- 不执行目标项目代码
- 不做 type inference / trait solving
- 不展开 macro
- 不执行 cargo metadata
