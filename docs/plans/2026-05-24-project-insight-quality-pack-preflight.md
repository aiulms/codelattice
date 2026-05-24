# Project Insight Quality Pack — Preflight

## Goal

Improve `codelattice_project(mode=insights)` so AI agents can navigate unfamiliar
projects more effectively. This pack focuses on insight quality, not new MCP tool
surface area.

## Scope

Implement three additional AI-oriented sections in project insights:

- `architectureMap`: compact module/component map derived from static file metrics.
- `suspiciousAreas`: ranked files/symbols that deserve inspection, with reasons and
  recommended actions.
- `missingEvidence`: explicit static-analysis gaps, so agents do not over-trust the
  output.

These sections should also pass through the `codelattice_project` facade in
`mode=insights`.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `CHANGELOG.md`
- `docs/plans/2026-05-24-project-insight-quality-pack-closure.md`

## Forbidden Set

- No CodeLattice-Tool sync or installed wrapper promotion.
- No live repo modification.
- No GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool edits.
- No AI client config edits.
- No target project code execution.

## Stop Lines

- Do not claim runtime diagnosis or root-cause proof.
- Do not add new MCP tools.
- Do not broaden full toolset by default.
- Do not treat static `suspiciousAreas` as test failure evidence.

## Verification Plan

- TDD focused tests for direct `codelattice_project_insights`.
- TDD focused test for `codelattice_project(mode=insights)` facade passthrough.
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test`
- Native `detect-changes`
