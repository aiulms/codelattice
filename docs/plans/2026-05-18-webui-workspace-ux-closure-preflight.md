# CodeLattice WebUI Workspace UX Closure Pack — Preflight

Date: 2026-05-18

## Goal

Close the most visible workspace-mode usability gaps after Workspace Inventory and Workspace Insights landed:

- keep users on the Workspace tab after bulk analysis instead of auto-opening the first child snapshot;
- let recommendation items open their related snapshot explicitly;
- make failed project rows explain the likely next action;
- show unsupported modules as a future language-support backlog;
- provide a compact workspace summary that users can copy into an AI assistant;
- make workspace reports respect the current Chinese/English UI language.

## Write Set

- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/runner.js`
- `webui/snapshot-viewer/report.js`
- `webui/snapshot-viewer/i18n.js`
- `webui/snapshot-viewer/styles.css`
- `scripts/webui-viewer-smoke.sh`
- `README.md`
- `CHANGELOG.md`
- `docs/webui/README.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-18-webui-workspace-ux-closure-closure.md`

## Forbidden Set

- No GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool changes.
- No AI client config changes.
- No target project source modification.
- No npm/pnpm/yarn/frontend framework introduction.
- No execution of target project scripts, builds, tests, Docker, or CI.

## Acceptance

- Viewer smoke includes explicit checks for workspace focus, insight snapshot open actions, AI summary copy, fix hints, unsupported backlog, i18n, and CSS.
- Workspace analysis completion keeps Workspace view active.
- Report export produces Chinese workspace headings when Chinese UI is active.
- Full WebUI smoke matrix and MCP baseline remain green.
