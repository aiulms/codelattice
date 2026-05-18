# WebUI Graph Showcase & Exploration Pack — Preflight

Date: 2026-05-18

## Goal

Turn the Graph tab from a useful but still tool-like view into a stronger exploration and presentation surface:

- keep the current G6/SVG fallback architecture;
- add one more visual layout aimed at large project screenshots;
- make hover/select/drilldown feel more informative;
- allow exporting the current graph view for sharing;
- keep wheel zoom lock and static-analysis cautions intact.

## Write Set

- `webui/snapshot-viewer/index.html`
- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/graph-g6.js`
- `webui/snapshot-viewer/styles.css`
- `webui/snapshot-viewer/i18n.js`
- `scripts/webui-viewer-smoke.sh`
- `docs/plans/README.md`
- `CHANGELOG.md`

## Forbidden Set

- No GitNexus-RC / GitNexus-RC-Tool changes.
- No CodeLattice-Tool promotion.
- No package manager / npm dependency.
- No target project writes or execution.
- No replacement of static graph semantics.

## Risk Notes

- `graph-g6.js::render` has HIGH blast radius because it is the main graph renderer for loaded snapshots.
- Changes to `app.js::renderGraphVisual` and `renderGraphNodeDetail` are LOW blast radius but directly affect the Graph tab.
- Mitigation: keep G6 API compatible (`CodeLatticeG6Graph.render(options)`), preserve SVG fallback, and expand smoke/browser verification.

## Acceptance

- Graph tab exposes a new `Heatmap` layout.
- Hovering/selecting nodes shows useful card context.
- Spotlight mode makes a larger presentation surface without losing interaction.
- Export button downloads current SVG/canvas graph image.
- Viewer smoke and browser smoke pass.
