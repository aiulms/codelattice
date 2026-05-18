# WebUI Graph Showcase Pack — Closure

Date: 2026-05-18

## Summary

This pack improves the Graph tab as a human-facing exploration and presentation surface. The goal was not to replace the static analysis backend, but to make the existing project graph more readable, clickable, and suitable for demo/social sharing screenshots.

## Delivered

- Added a new **Module Heatmap** layout for package/file/symbol clustering.
- Added graph showcase overlay with node/edge/call/file/symbol metrics.
- Added node hover cards with kind, file, degree, call count, and interaction hints.
- Added **Spotlight Mode** to hide side panels and make the graph the primary viewport.
- Added graph PNG export support for G6 canvas and SVG fallback.
- Extended bilingual labels for the new graph controls.
- Extended WebUI smoke coverage for heatmap, spotlight, export, hover card, and showcase CSS.

## Validation

- `node -c webui/snapshot-viewer/app.js`
- `node -c webui/snapshot-viewer/graph-g6.js`
- `node -c webui/snapshot-viewer/i18n.js`
- `bash scripts/webui-viewer-smoke.sh --skip-browser`
- `bash scripts/webui-i18n-smoke.sh`
- Browser runtime check against local runner on a real ArkTS snapshot confirmed G6 heatmap + spotlight rendering.

## Boundaries

- No GitNexus-RC changes.
- No GitNexus-RC-Tool changes.
- No CodeLattice-Tool changes.
- No AI client config changes.
- No real project source changes.
- No new frontend package manager or npm dependency.

## Notes

`graph-g6.js::render` is a high-impact WebUI render path. Changes were kept additive: SVG fallback remains, the G6 adapter still owns the advanced renderer, and the new visual controls reuse existing graph state.
