# Project Insight Quality Pack — Closure

## Summary

This pack improves project insights for AI agents without adding MCP tools or
changing the facade model.

Implemented:

- `architectureMap` in `codelattice_project_insights`, grouping files into static
  components with role, counts, risk level, read-first files, and recommended action.
- `suspiciousAreas`, converting risk map signals into AI-readable inspection targets
  with `whySuspicious`, `recommendedAction`, and `staticOnly`.
- `missingEvidence`, explicitly stating unavailable runtime, coverage, type inference,
  entry-point proof, and sparse graph evidence where relevant.
- Facade passthrough for `codelattice_project(mode=insights)` when non-compact result
  detail is requested.

## Safety

- Static analysis only.
- No target project code execution.
- No new MCP tools.
- No CodeLattice-Tool sync/promotion.
- No live repo modification.
- No production/readiness proof claims.

## Verification

TDD checks were run red/green. Before implementation, both focused tests failed
because project insights did not expose the new AI navigation sections. After
implementation:

- `cargo test --test mcp_server mcp_project_insights_returns_ai_navigation_sections -- --nocapture` — pass
- `cargo test --test mcp_server mcp_project_facade_insights_preserves_ai_navigation_sections -- --nocapture` — pass

Full verification:

- `cargo fmt --check` — pass
- `git diff --check` — pass
- `cargo test --test mcp_server` — 147 passed, 0 failed
- `cargo test` — pass
- `target/debug/codelattice detect-changes --root . --language rust --scope all --compact` — completed

Native detect-changes reported `summary.riskLevel=critical` and
`crossProjectRisk=critical`. This is expected for this pack because it changes MCP
project-insight runtime output in `crates/cli/src/mcp_server.rs`, which is a
cross-project facade path. The changed symbols themselves were reported as `LOW`;
the critical rating comes from workspace-wide blast-radius policy, unknown doc/test
hunks, and unsupported fixture boundary summaries. The risk was mitigated with
focused TDD tests, full MCP server tests, and full workspace tests.
