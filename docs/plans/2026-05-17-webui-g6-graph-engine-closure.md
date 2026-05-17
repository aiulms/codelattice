# WebUI G6 Graph Engine Closure

Date: 2026-05-17
Commit: finalized in associated G6 graph engine commit

## Summary

AntV G6 is now the default advanced graph renderer for the CodeLattice WebUI Graph tab. The existing SVG renderer remains available as a compatibility fallback.

## Delivered

- Vendored `@antv/g6` 5.1.1 browser bundle under `webui/snapshot-viewer/vendor/g6/`
- Added `webui/snapshot-viewer/graph-g6.js` adapter
- Added Graph engine selector:
  - `G6 高级图谱`
  - `SVG 兼容图谱`
- Preserved existing graph layout modes:
  - 代码星云
  - 模块星团
  - 调用流向
  - 蓝图架构
  - 工程探索
- Added G6 canvas rendering with:
  - drag canvas
  - zoom canvas
  - drag nodes
  - click select
  - double-click drill-down
- Added G6-specific visual styling and runtime badge
- Updated viewer/browser/i18n smoke checks

## Boundaries

- No npm / pnpm / yarn project setup
- No React / Vue / Svelte / desktop shell
- No MCP schema or CLI output changes
- No runtime proof claims
- SVG renderer remains as fallback

## Verification

- `node -c` for `graph-g6.js`, `app.js`, and vendored `g6.min.js`
- `scripts/webui-viewer-smoke.sh --skip-browser`: 96/96 pass
- `scripts/webui-i18n-smoke.sh`: 26/26 pass
- `scripts/webui-browser-smoke.sh`: 16 pass, 0 fail, 1 skipped
- `scripts/webui-beta-sanity.sh`: pass
- `cargo fmt --check`: pass
- `git diff --check`: pass
- `cargo test --test mcp_server`: 114/114 pass
- `scripts/codelattice-mcp.sh --self-test`: pass
- `scripts/mcp-dogfood.sh`: 37/37 pass
- GitNexus detect-changes: medium risk, WebUI graph-rendering scope
- GitNexus index refresh: 8,286 nodes / 15,258 edges / 170 clusters / 300 flows
- Browser runtime check:
  - `window.G6 === true`
  - `window.CodeLatticeG6Graph.available() === true`
  - Graph tab rendered G6 canvas layers
  - selecting a node updated detail panel

## Residual Risk

G6 is a large vendored bundle (~1.38MB). This is acceptable for the local WebUI workbench, but future release packaging should mention the vendored MIT dependency.
