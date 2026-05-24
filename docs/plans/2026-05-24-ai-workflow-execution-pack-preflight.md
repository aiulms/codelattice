# CodeLattice AI Workflow Execution Pack Preflight

Date: 2026-05-24

## Goal

Make `codelattice_workflow(..., execute=true)` easier for AI agents to use as a single investigation entry point. The workflow should not only run recommended facade actions, but also return a structured investigation report that explains what was checked, what evidence was found, what remains unproven, and what a human/agent should verify next.

## Scope

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `CHANGELOG.md`
- `docs/plans/2026-05-24-ai-workflow-execution-pack-closure.md`

Forbidden set:

- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool runtime
- AI client configuration
- live repositories such as `open-nwe` and `cangjie`

## Design

Keep the existing six-tool AI facade surface. Extend the existing workflow executor with additive fields:

- `investigationPlan`
- `aiDecisionTrace`
- `evidenceFound`
- `evidenceMissing`
- `humanVerificationNeeded`

The output remains static-only. No target project code is executed. Missing required inputs must continue to stop eager execution and preserve discovery actions.

## Stop Lines

- Do not add new MCP tools.
- Do not run workflow actions in parallel inside MCP stdio.
- Do not treat static findings as runtime, test, coverage, or deletion proof.
- Do not sync the installed `CodeLattice-Tool` copy in this pack.

## Verification

- Focused MCP workflow tests.
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test`
- Native detect-changes or precommit governance before commit.
