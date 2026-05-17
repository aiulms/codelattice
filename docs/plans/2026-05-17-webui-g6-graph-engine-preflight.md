# WebUI G6 Graph Engine Preflight

Date: 2026-05-17
Scope: CodeLattice WebUI graph rendering upgrade

## Goal

Upgrade the WebUI graph view from the current static SVG renderer to an AntV G6-powered interactive graph engine while keeping the current SVG renderer as a fallback.

The goal is visual and interaction quality for human users:

- better poster-quality graph visuals
- zoom / pan / drag interaction
- click and hover neighbor highlighting
- double-click drill-down
- multiple presentation layouts
- no change to MCP, CLI, or analysis semantics

## Write Set

- `webui/vendor/g6/`
- `webui/snapshot-viewer/graph-g6.js`
- `webui/snapshot-viewer/index.html`
- `webui/snapshot-viewer/app.js`
- `webui/snapshot-viewer/styles.css`
- `webui/snapshot-viewer/i18n.js`
- `scripts/webui-viewer-smoke.sh`
- `docs/webui/`
- `CHANGELOG.md`

## Forbidden Set

- GitNexus-RC
- GitNexus-RC-Tool
- CodeLattice-Tool stable runtime
- user AI client configs
- live project repositories
- MCP schema or tool output semantics

## Design

AntV G6 is vendored as a static browser bundle. CodeLattice keeps ownership of code-graph semantics and layout decisions:

- node/edge ranking
- semantic colors
- file/symbol/package grouping
- drill-down and focus semantics
- SVG fallback

G6 is used only as the rendering and interaction engine.

## Stop-Lines

- Do not introduce npm/pnpm/yarn project setup.
- Do not require network access at runtime.
- Do not remove the current SVG fallback.
- Do not claim runtime proof or compiler verification.
- Do not render all full-project nodes without top-N bounds.

## Verification

- JS syntax checks
- WebUI viewer smoke
- i18n smoke
- runner smoke
- live MCP smoke
- browser render smoke for G6 canvas
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
