# Architecture Insight Quality Pack — Preflight

## Goal

Improve CodeLattice MCP insight quality for AI agents without changing the installed `CodeLattice-Tool` distribution in this round.

The immediate feedback to address:

- `project_insights` ranks test functions as top entry points.
- `risk` is too often flat `LOW` and does not explain architecture readiness dimensions.
- `sourceOnly` workspace entries are counted but not explained.
- AI agents need clearer next actions for onboarding and problem localization.

## Scope

Allowed writes:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-24-architecture-insight-quality-preflight.md`
- `CHANGELOG.md`

Forbidden:

- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool/`
- live repos such as `open-nwe` / `cangjie`
- GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool
- AI client configuration files

## Implementation Shape

1. Add source-only explanation objects for workspace auto-entry and root diagnosis.
2. Replace raw fan-out entry ranking in `project_insights` with classified entry signals:
   runtime start, framework/route, CLI, package/public API, orchestration, test entry.
3. Down-rank test-like symbols and paths so they do not dominate top architecture entry points.
4. Add architecture risk dimensions and next actions to `project_insights`.
5. Add MCP regression tests for the new fields and ranking policy.

## Risk

Risk is medium because `mcp_server.rs` is central to the AI MCP facade. The change stays inside static analysis output shaping and does not execute target project code.

Validation:

- `cargo fmt --check`
- `git diff --check`
- focused MCP integration tests
- `target/debug/codelattice detect-changes --root . --language rust --scope all --compact`

