# AI Tool Experience Pack — Closure

Date: 2026-05-20
Status: Completed

## Summary

`codelattice_workflow` now acts as the default AI intent router inside the small MCP `ai` toolset. It returns an `ai.workflow.v1` envelope with actionable `nextActions`, missing-input guidance, and static-analysis cautions.

## Supported Intents

- `onboarding`
- `before_edit`
- `after_edit`
- `delete_code`
- `release_check`
- `legacy_cleanup`
- `workspace_review`
- `cross_project_impact`
- `explain_symbol`
- `docs_tests_sync`
- `config_examples_sync`
- `public_api_change`
- `framework_route_change`

## Behavior

- `before_edit` with a `symbol` routes to `codelattice_symbol mode=context`, `codelattice_change_review mode=impact`, and caller inspection.
- `before_edit` without a `symbol` does not fail; it returns `missingInputs` and routes to a directly callable `codelattice_symbol mode=search` action with `query` present.
- `cross_project_impact` without a `target` does not fail; it routes to `codelattice_workspace mode=graph` so the agent can choose a stable target.
- `delete_code` is always high caution and returns `safeToProceed="no"`.
- All workflow outputs keep `generatedFrom.staticAnalysis=true`, `runtimeVerified=false`, `scriptsExecuted=false`, and `coverageVerified=false`.

## Verification

- `cargo test --test mcp_server mcp_workflow_ -- --nocapture`: PASS, 13/13
- `cargo test --test mcp_server`: PASS, 126/126
- `bash scripts/codelattice-mcp-facade-smoke.sh`: PASS, 12/12
- `bash scripts/mcp-real-client-dry-run.sh`: PASS, 11/11
- `bash scripts/codelattice-precommit-check.sh`: PASS

Native detect-changes reported `critical` because this pack changes MCP routing core, facade smoke behavior, and documentation. The risk was reviewed before commit: tests and smoke passed, and the high workspace blast-radius signal is expected for a default AI workflow router change.

## Boundaries

- GitNexus-RC: not touched
- GitNexus-RC-Tool: not touched
- CodeLattice-Tool stable install: not touched
- AI client config: not touched
- Real project source: not touched
