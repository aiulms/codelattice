# Live Root Workspace Auto-Entry Pack — Preflight

Date: 2026-05-20

## Goal

让 CodeLattice 接受“仓库/工作区根目录”作为自然入口。用户或 AI 不需要先猜哪个子目录可分析；当根目录包含多个可分析子项目时，CodeLattice 自动进入 workspace 模式，返回支持项目、暂不支持模块、workspace graph 摘要和后续建议。

## Problem

旧行为对两类场景不友好：

- WebUI 用户选择一个大目录后，`auto` 分析可能直接尝试分析根目录，然后因为多语言 manifest 或 live-root 保护失败。
- AI agent 面对 protected live root 时，需要自己知道 “切到 runtime/cjgui” 这类内部约定，工具使用成本高。

## Scope

写入范围：

- `crates/cli/src/lib.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/`
- `scripts/webui-runner.py`
- `scripts/webui-workspace-smoke.sh`
- `scripts/webui-workbench-trial.sh`
- `webui/snapshot-viewer/runner.js`
- `README.md`
- `docs/architecture/mcp-v0-contract.md`
- `docs/plans/`

禁止范围：

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- CodeLattice-Tool stable install
- live repo 源码
- AI client 实际配置

## Design

1. CLI `analyze --language auto` 在非 bridge 输出路径上，先做 workspace inventory；如果发现 2 个以上可支持子项目，返回 `codelattice.workspaceAutoEntry.v1`，不进入单语言分析。
2. MCP facade `codelattice_project` 在 `language=auto` 时优先返回 workspace auto-entry；低层 `codelattice_analyze` 对 protected live root 继续保持 path-denied，避免无意中裸扫 live repo。
3. `codelattice_workspace_graph` / `codelattice_cross_project_impact` 允许 protected root 作为只读 workspace root 使用，并在输出中标记 `liveRootProtected=true`。
4. WebUI Runner `quick-analyze` 对 multi-project root 自动执行 workspace recommended analysis，返回 `kind=workspace` 和 `workspaceId`。
5. WebUI 前端一键分析大目录时自动进入 Workspace 总览，不要求用户手工选子目录。

## Safety

- 只读目录结构、manifest/config 文件名和已有 workspace graph 需要的静态输入。
- 不执行目标项目代码、构建脚本或测试。
- protected root 的低层单项目 analyze 仍然被拒绝；只开放 workspace inventory/graph/impact/auto-entry。
- 所有输出继续标注 static-only / runtimeVerified=false / scriptsExecuted=false。

## Acceptance

- CLI 多项目根目录自动返回 workspace auto-entry JSON。
- MCP `codelattice_project(mode=overview, language=auto)` 对多项目根目录返回 workspace auto-entry。
- WebUI `POST /api/quick-analyze` 对多项目根目录返回 `kind=workspace` 并创建 workspace run。
- Workspace smoke 和 workbench trial 覆盖新行为。
