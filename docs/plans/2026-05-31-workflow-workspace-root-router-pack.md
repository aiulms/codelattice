# Workflow Workspace Root Router Pack

Date: 2026-05-31

## Goal

Let AI agents call `codelattice_workflow` with a workspace root for common
symbol-level workflows when a target symbol/query is already present.

## Why Now

`codelattice_symbol` and `codelattice_change_review` can now route a workspace
root to the matching project root. The workflow facade still has a guard that
stops symbol-level workflows at workspace roots and asks the AI to choose a
project manually. That leaves the main AI entry point less capable than the
specific tools underneath it.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- this plan

## Forbidden Set

- Do not change MCP tool counts.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not edit live repositories such as `open-nwe`.
- Do not make workflow routing execute target code or package-manager scripts.

## Design

### 1. Reuse the root router

When `codelattice_workflow` receives a workspace root for a symbol-level
workflow, reuse `workspace_auto_route_decision` before applying the old
`projectRoot` missing-input guard.

### 2. Rewrite nextActions, not caller intent

Keep the top-level `root` as the original caller root and add a top-level
`rootRouter`. Rewrite generated `nextActions` so project/symbol/change-review
actions use the selected concrete project root and selected language. Keep
workspace-level actions on the original workspace root.

### 3. Fall back safely

If the router cannot confidently choose a project, keep the existing
workspace-level guidance: `missingInputs.projectRoot`, workspace graph action,
and optional workspace impact action.

## Stop Lines

- Stop if default AI toolset changes from 6 or full toolset changes from 49.
- Stop if `workflow(before_edit, workspace root, symbol, execute=true)` stops at
  missing `projectRoot` when a child project clearly contains the symbol.
- Stop if workflow auto-routing silently changes workspace-level actions into
  project-level actions.
- Stop if compact output expands full source-only lists.

## Verification

- Add a failing MCP test where `codelattice_workflow(mode=before_edit,
  execute=true)` receives a workspace root and symbol that exists only in a
  child Rust project.
- Verify response includes `rootRouter.routed=true`.
- Verify generated project/symbol/change-review nextActions use the selected
  child root and language.
- Verify execution completes and runs both symbol context and impact actions.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`,
  `cargo test`, `scripts/codelattice-installed-acceptance.sh --dev-only`, and
  `scripts/codelattice-precommit-check.sh`.
