# CodeLattice AI Progressive Exploration Pack — Closure

## Summary

This pack makes the six-tool MCP surface easier for AI agents to use progressively. Instead of asking agents to choose between broad `overview`, deep `insights`, or large `full` payloads, CodeLattice now exposes a small depth model inside the existing facades:

- `codelattice_project(mode=quick)` for first contact.
- `codelattice_project(mode=standard)` for component and risk review.
- `codelattice_project(mode=deep)` for full static evidence.
- `codelattice_workflow(mode=explore, depth=1..3)` for guided project/workspace orientation.

No new MCP tools were added.

## Behavior

- Workspace auto-entry now emits `primaryProjectRoots` with rank, score, `whyRecommended`, and next action.
- Workspace compact responses limit noisy `sourceOnlyEntries` while preserving counts and summary.
- Compact facade responses keep useful `summary` data and add `detailHint`, instead of reducing summary to only `riskLevel`.
- Project progressive modes return `summary.aiDigest` with `readFirst`, `reviewFirst`, `entryPoints`, `topComponents`, `topRisks`, `missingEvidence`, and `drillDownOptions`.
- Workflow explore produces an `explorationPlan` with objective, read-first guidance, drill-down options, and next queries.

## Verification

- `cargo test --test mcp_server mcp_project_workspace_auto_entry_prioritizes_main_projects -- --nocapture`
- `cargo test --test mcp_server mcp_project_quick_returns_compact_ai_digest -- --nocapture`
- `cargo test --test mcp_server mcp_workflow_explore_routes_progressive_project_map -- --nocapture`
- `cargo test --test mcp_server`
- `cargo test`
- `scripts/codelattice-precommit-check.sh`

All listed checks passed in the dev workspace before closure.

Native `detect-changes` reported critical cross-project static risk because this pack changes MCP facade routing, compact response behavior, and workflow output. This was reviewed as expected blast radius for an MCP UX/runtime surface change, not as evidence of runtime failure. Full Rust test coverage, MCP regression, concurrency smoke, and detect-changes smoke completed successfully.

## Boundaries

- Static analysis only; no target project code is executed.
- No parser or language semantics changes.
- No CodeLattice-Tool sync in this pack.
- No AI client configuration changes.
