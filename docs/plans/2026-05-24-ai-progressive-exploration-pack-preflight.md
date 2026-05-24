# CodeLattice AI Progressive Exploration Pack — Preflight

## Goal

Make CodeLattice easier for AI agents to use as a first-pass project guide, not just a large static-analysis dump.

## Scope

- Add progressive project facade modes: `quick`, `standard`, `deep`.
- Add `codelattice_workflow(mode=explore)` for depth-based exploration.
- Improve compact facade behavior so summaries stay useful while full payloads are omitted.
- Rank workspace project roots so AI agents know which subproject to analyze first.

## Non-goals

- No new parser semantics.
- No target project execution.
- No CodeLattice-Tool sync in this pack unless requested after validation.
- No AI client config edits.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/`
- `CHANGELOG.md`

## Risk

- Medium: public MCP facade schemas and compact behavior change.
- Mitigation: focused MCP tests for quick/explore/workspace ranking plus full MCP regression.

## Stop Lines

- Static analysis remains non-runtime proof.
- Compact responses must not hide next actions.
- Workspace root handling must guide to subprojects instead of pretending symbol-level analysis is complete.
