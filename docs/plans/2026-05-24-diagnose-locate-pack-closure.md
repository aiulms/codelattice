# Diagnose / Locate Pack — Closure

## Summary

This pack adds an AI-oriented issue location path without increasing MCP tool count.

Implemented:

- `codelattice_project(mode=diagnose)` for static issue localization.
- `codelattice_workflow(mode=diagnose_issue)` as a router for agents that have a symptom/error/query but do not know which tool to call.
- Ranked `likelyAreas` with confidence, reason, fan-in/fan-out, entry kind, and next action.
- Concrete `readFirst` ordering so agents know which files to inspect first.
- `entryPoints` and `impactHints` sections to connect diagnosis to architecture and blast-radius review.
- Input signal extraction from `symptom`, `errorText`, `query`, `changedPath`, `symbol`, `name`, `issue`, and `observedError`.

## Safety

- Static analysis only.
- No target project code execution.
- No runtime/root-cause proof claims.
- No new MCP tools.
- No CodeLattice-Tool sync/promotion.
- No live repo modification.

## Verification

TDD checks were run red/green. Before implementation, both focused tests failed because
`diagnose` / `diagnose_issue` were not accepted modes. After implementation:

- `cargo test --test mcp_server mcp_project_diagnose_returns_ranked_likely_areas -- --nocapture` — pass
- `cargo test --test mcp_server mcp_workflow_diagnose_issue_routes_to_project_diagnose -- --nocapture` — pass

Full verification:

- `cargo fmt --check` — pass
- `git diff --check` — pass
- `cargo test --test mcp_server` — 145 passed, 0 failed
- `cargo test` — pass
- `target/debug/codelattice detect-changes --root . --language rust --scope all --compact` — completed

Native detect-changes reported `summary.riskLevel=critical` and
`crossProjectRisk=critical`. This is expected for this pack because the change touches
MCP runtime/facade routing (`crates/cli/src/mcp_server.rs`) and exposes a new agent
workflow path. The changed symbols themselves were reported as `LOW`; the critical
rating comes from workspace-wide blast-radius policy, unknown doc/test hunks, and
unsupported fixture boundary summaries. The risk was mitigated with focused TDD tests,
the full MCP server suite, and the full workspace test suite.
