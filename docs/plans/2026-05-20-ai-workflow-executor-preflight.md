# AI Workflow Executor Pack — Preflight

Date: 2026-05-20
Status: Approved for implementation

## Goal

Make CodeLattice easier for AI agents by allowing `codelattice_workflow` to execute its own recommended non-recursive facade actions when `execute=true`.

## Scope

- Extend the existing `codelattice_workflow` facade; do not add new MCP tools.
- Keep `execute=false` as the default router-only behavior.
- Add execution metadata:
  - `execution`
  - `completedActions`
  - `failedActions`
  - `skippedActions`
  - `evidence`
  - `answerSummary`
- Stop safely with `execution.status=needs_input` when required inputs such as `symbol` or `target` are missing.
- Keep static-only cautions and no runtime/build/test execution.

## Execution Card

Write set:
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-mcp-facade-smoke.sh`
- `README.md`
- `CHANGELOG.md`
- `docs/architecture/mcp-v0-contract.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-20-ai-workflow-executor-preflight.md`
- `docs/plans/2026-05-20-ai-workflow-executor-closure.md`

Forbidden set:
- GitNexus-RC
- GitNexus-RC-Tool
- CodeLattice-Tool stable install
- AI client configuration
- Real project source trees

## Risk

Risk is medium-high because the handler is central to the default AI MCP entrypoint. Mitigation is TDD on workflow execution, facade smoke, full MCP regression, and native precommit governance.

