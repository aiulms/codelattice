# WebUI Visual Identity Pack Preflight

Date: 2026-05-18

## Goal

Upgrade the WebUI from a function-by-function utility surface into a more coherent product workbench. The visual direction references the immersive data-field feeling of modern AI product pages, but the implementation remains a practical CodeLattice analysis tool rather than a marketing landing page.

## Scope

- Refresh the first-run project picker into an immersive welcome stage.
- Add a loaded-project hero that anchors the post-analysis workbench.
- Replace visible emoji controls with lightweight CSS icons and status dots.
- Unify panels, tabs, cards, buttons, and dashboard surfaces around one visual system.
- Preserve all existing runner, snapshot, graph, MCP, i18n, and static-only behavior.
- Extend smoke coverage so the visual shell does not regress silently.

## Write Set

- `webui/snapshot-viewer/index.html`
- `webui/snapshot-viewer/styles.css`
- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/runner.js`
- `webui/snapshot-viewer/i18n.js`
- `scripts/webui-viewer-smoke.sh`
- `scripts/webui-i18n-smoke.sh`
- `docs/plans/`
- `CHANGELOG.md`

## Stop-Lines

- No new frontend framework or package manager.
- No change to MCP tool semantics, snapshot schema, or graph data shape.
- No backend code execution beyond existing runner behavior.
- No edits outside the CodeLattice repository.
- Do not remove static-analysis cautions.

## Risk

`renderHeader` and related loaded-state paths are broad UI render paths. Changes must stay additive and visual: fill new hero metric fields, update body state classes, and preserve existing tab/load/error behavior.

## Acceptance

- JS syntax passes.
- `webui-viewer-smoke.sh --skip-browser` passes with visual shell checks.
- `webui-i18n-smoke.sh` passes with new hero/workbench translation keys.
- Browser screenshot confirms no obvious title overlap or favicon error.
- Standard CodeLattice verification passes before commit.
