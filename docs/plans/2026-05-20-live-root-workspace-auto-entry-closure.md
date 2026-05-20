# Live Root Workspace Auto-Entry Pack — Closure

Date: 2026-05-20

## Result

CodeLattice 现在把大根目录作为一等入口处理：当 `auto` 语言检测发现根目录包含多个可分析子项目时，CLI、MCP facade 和 WebUI Runner 都会自动进入 workspace 模式，而不是要求用户或 AI 先找具体子目录。

## Shipped

- CLI `analyze --language auto`：多项目根目录返回 `codelattice.workspaceAutoEntry.v1`。
- MCP `codelattice_project`：`language=auto` 多项目根目录返回 workspace auto-entry。
- MCP workspace graph/impact：允许 protected live root 以只读 workspace root 进入，并标注 `liveRootProtected=true`。
- WebUI Runner `quick-analyze`：multi-project root 自动执行 workspace recommended analysis，返回 `kind=workspace`。
- WebUI 前端：Project Picker / Generate 路径在 multi-project root 上自动切换到 Workspace 总览。
- Smoke：`webui-workspace-smoke.sh` 与 `webui-workbench-trial.sh` 覆盖 quick-analyze workspace auto-entry。

## Safety Review

- 低层 `codelattice_analyze` 对 protected live root 继续 path-denied。
- 本次只放开 workspace inventory/graph/impact/auto-entry，不放开裸扫 live root。
- WebUI Runner 仍写入 `.codelattice-webui/`，不写目标项目。
- 所有新输出标记 static-only，`scriptsExecuted=false`。

## Verification

已执行：

- `python3 -m py_compile scripts/webui-runner.py`
- `node --check webui/snapshot-viewer/runner.js`
- `cargo test --test productization_commands analyze_auto_multi_project_root_returns_workspace_auto_entry -- --nocapture`
- `cargo test --test mcp_server mcp_project_auto_enters_workspace_for_multi_project_root -- --nocapture`
- `bash scripts/webui-workspace-smoke.sh`
- `bash scripts/webui-workbench-trial.sh`
- `bash scripts/webui-viewer-smoke.sh --skip-browser`
- `bash scripts/codelattice-mcp.sh --self-test`
- `scripts/codelattice-precommit-check.sh`
- `cargo test`

Native precommit 通过，但 `detect-changes` 对入口路由类改动给出 `critical` 跨项目风险：影响 CLI/MCP/Runner 入口，并触达 workspace graph 下游与 fixture unsupported 边界。已通过 productization、MCP、WebUI workspace/workbench、viewer smoke 和全量 `cargo test` 覆盖后继续提交。

## Known Limits

- Auto-entry 只在 `language=auto` 生效；显式语言仍按用户指定执行。
- Protected root 下只允许 workspace 层面的静态发现/图谱/影响入口；单项目 analyze 仍需选择具体子项目。
- Workspace auto-entry 是静态结构判断，不是构建成功或运行时覆盖证明。
