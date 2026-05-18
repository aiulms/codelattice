# WebUI Alpha Usability Pack — Closure

Date: 2026-05-18

## Scope

This pack focused on the first-run WebUI path for real local projects:

- selecting a broad workspace root should not dead-end with a vague generation error;
- mixed repositories should show concrete supported subprojects;
- unsupported languages should be visible as unsupported, not silently ignored;
- graph exploration should provide clickable relationship context after selecting a node.

## Delivered

- Added runner project inventory API: `GET /api/project/inventory?root=...`.
- Added supported-language inventory for Rust, Cangjie, ArkTS, TypeScript, C, C++, Python, plus unsupported C# detection.
- Added Project Radar UI on both the first screen and the loaded workbench panel.
- Added automatic candidate handling before one-click analysis:
  - root project: analyze directly;
  - single supported child project: analyze the child;
  - multiple child projects: ask the user to choose;
  - unsupported-only / empty roots: show actionable explanation.
- Added graph detail drilldown sections:
  - incoming relationships;
  - outgoing relationships;
  - clickable neighbor rows that focus the related node.
- Updated smoke/trial scripts for real-project root selection and unsupported-language handling.

## Real UI Checks

- `/Users/jiangxuanyang/Desktop/cangjie` now returns `multi_project` with 16 supported candidate projects instead of a raw analyze failure.
- Existing Cangjie snapshot `cangjie_stdx_1.1` opens in Graph view with 142 nodes, 164 edges, and 21 call edges.
- Selecting a graph node renders incoming/outgoing relationship sections for drilldown.

## Boundaries

- No GitNexus-RC / GitNexus-RC-Tool changes.
- No CodeLattice-Tool promotion.
- No AI client configuration changes.
- No writes to analyzed target projects.
- No execution of target project code.

## Verification

- `bash scripts/webui-viewer-smoke.sh --skip-browser`
- `bash scripts/webui-i18n-smoke.sh`
- `bash scripts/webui-workbench-trial.sh`
- `python3 -m py_compile scripts/webui-runner.py`
- `node -c webui/snapshot-viewer/{app.js,runner.js,i18n.js}`

Final full verification was run before commit.
