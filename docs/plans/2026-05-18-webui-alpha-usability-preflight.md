# WebUI Alpha Usability / Real Project Trial Preflight

## Goal

Make the WebUI first-run flow usable on real repositories: choose a project, understand whether CodeLattice can analyze it, avoid ambiguous monorepo failures, inspect a graph node, and recover from unsupported language modules without guessing.

## Write Set

- `scripts/webui-runner.py`
- `scripts/webui-viewer-smoke.sh`
- `scripts/webui-workbench-trial.sh`
- `webui/snapshot-viewer/index.html`
- `webui/snapshot-viewer/runner.js`
- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/i18n.js`
- `webui/snapshot-viewer/styles.css`
- `CHANGELOG.md`
- `docs/plans/README.md`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- CodeLattice-Tool stable runtime
- AI client configs
- User live repos and source trees

## Design

1. Add a runner-side project inventory endpoint that scans only filenames and directories. It reports supported root languages, candidate subprojects, unsupported modules such as C#, and a recommendation.
2. Add Project Radar panels to the first screen and loaded workbench runner panel. When the selected path is a monorepo or unsupported-only directory, the UI shows actionable candidate buttons instead of starting a doomed analysis.
3. Improve graph node detail so clicking a node explains incoming/outgoing relationships and provides direct neighbor drilldown buttons.
4. Extend workbench smoke/trial to cover project inventory, monorepo candidates, unsupported language reporting, and the graph drilldown UI.

## Stop-lines

- Do not add new language analyzers.
- Do not pretend unsupported languages are analyzed.
- Do not execute user project code.
- Do not write to selected projects.
- Keep WebUI local-only and dependency-free beyond already vendored G6.

## Verification

- `cargo fmt --check`
- `git diff --check`
- `bash scripts/webui-viewer-smoke.sh --skip-browser`
- `bash scripts/webui-i18n-smoke.sh`
- `bash scripts/webui-workbench-trial.sh`
- `bash scripts/webui-browser-smoke.sh`
- `cargo test --test mcp_server`
- `bash scripts/codelattice-mcp.sh --self-test`
- `bash scripts/mcp-dogfood.sh`
- GitNexus `detect-changes`
- GitNexus index refresh
