# AI Tool Experience Pack — Preflight

Date: 2026-05-20
Status: Completed in same work package

## Goal

Make the default MCP `ai` toolset easier for coding agents to use by turning `codelattice_workflow` into a real intent router. The tool should guide the next step with directly callable `nextActions` instead of only returning preset documentation.

## Scope

- Enhance the existing `codelattice_workflow` facade.
- Keep the default AI toolset small; do not add more top-level MCP tools.
- Support missing-input guidance for common AI dead ends:
  - before edit without a symbol
  - cross-project impact without a target
  - explain symbol without a symbol/name
- Return a stable AI action envelope:
  - `schemaVersion`
  - `situation`
  - `riskLevel`
  - `confidence`
  - `findings`
  - `missingInputs`
  - `nextActions`
  - `cautions`
  - `humanReviewNeeded`
  - `safeToProceed`
  - `generatedFrom`

## Stop-lines

- Do not execute target project code.
- Do not claim runtime, compiler, coverage, external usage, or deletion proof.
- Do not expand the default MCP tool count beyond the existing compact AI surface.
- Do not remove existing facade tools or full/core compatibility.

## Verification Plan

- Add MCP regression tests for:
  - `before_edit` with symbol returns callable impact/context next actions.
  - `before_edit` without symbol returns `missingInputs` and symbol-search next action.
  - `cross_project_impact` without target returns workspace-graph next action.
- Update `scripts/codelattice-mcp-facade-smoke.sh` to cover the AI action envelope.
- Run:
  - `cargo fmt --check`
  - `git diff --check`
  - `cargo test --test mcp_server`
  - `bash scripts/codelattice-mcp-facade-smoke.sh`
  - `bash scripts/codelattice-precommit-check.sh`

