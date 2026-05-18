# WebUI Automation Graph Integration Preflight

Date: 2026-05-18

## Goal

把 `codelattice_automation_graph` 从 MCP 工具接入 WebUI 工作台，让人类在 Release / Workflow / Live MCP / Report 路径里能看到自动化工作流、步骤和风险候选，而不是只在 MCP JSON 里查看。

## Write Set

- `scripts/webui-runner.py`
- `scripts/webui-viewer-smoke.sh`
- `scripts/webui-live-mcp-contract-test.sh`
- `scripts/webui-runner-smoke.sh`
- `webui/snapshot-viewer/index.html`
- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/live.js`
- `webui/snapshot-viewer/report.js`
- `webui/snapshot-viewer/i18n.js`
- `webui/snapshot-viewer/styles.css`
- README / CHANGELOG / docs

## Stop-lines

- 不执行目标项目脚本、CI、Docker、Makefile 或 package scripts。
- 不把自动化风险候选表述为运行时证明。
- MCP 不可用时不能阻断普通 snapshot 生成。
- 不引入 npm / 前端框架 / 新网络依赖。

## Acceptance

- Viewer smoke 先新增失败断言，再实现到全绿。
- Runner snapshot detail 包含 `automationGraph` 或明确 `not_collected`。
- Live MCP contract 创建并验证 `automation_graph` job。
- Release / Workflow / Report 均能消费自动化图谱摘要。
