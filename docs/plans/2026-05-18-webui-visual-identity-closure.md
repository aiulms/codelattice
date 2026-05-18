# WebUI Visual Identity Pack Closure

Date: 2026-05-18

## Summary

The WebUI visual system was upgraded around one product workbench direction: an immersive first-run code-map stage, a loaded-project hero, glassy analysis panels, compact CSS icons, and smoke checks for the new shell. This is a visual and usability pack only; MCP tools, snapshot schema, graph semantics, runner behavior, and analysis output remain unchanged.

## Delivered

- First-run welcome screen now uses a larger CodeLattice-native hero with proof metrics for supported languages, MCP tools, and local-only operation.
- Loaded snapshots now show a workbench hero with language/files/symbols/edge metrics before the tab surface.
- Header badges and action buttons use CSS-drawn icons/status dots instead of visible emoji controls.
- Panels, cards, tabs, inputs, buttons, dashboard cards, and graph containers share the same glass/grid visual language.
- Added an inline SVG favicon to avoid local-browser favicon 404 noise.
- Added smoke checks for the premium welcome shell, workbench hero, CSS icons, favicon, data-field background, and Chinese title fitting.
- Added i18n smoke coverage for hero/workbench/dashboard edge labels.

## Verification

- `node -c` passed for the touched WebUI scripts.
- `bash scripts/webui-viewer-smoke.sh --skip-browser` passed with 126/126 checks.
- `bash scripts/webui-i18n-smoke.sh` passed with 40/40 checks.
- Browser smoke screenshot was checked manually with Playwright; the Chinese title no longer creates a one-character dangling line or overlaps the project card.

## Boundaries

- No GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool changes.
- No AI client config changes.
- No new npm package or frontend framework.
- No snapshot schema or MCP tool behavior changes.
