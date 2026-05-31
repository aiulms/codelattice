# Ask Workspace Root Router Pack

Date: 2026-05-31

## Goal

Make `codelattice_workflow(mode=ask)` as forgiving as the other AI facade entry points when the user passes a workspace root. Natural-language ask is the most likely first tool call for an AI agent, so it must not require the agent to manually convert workspace roots into concrete project roots before answering symbol-level questions.

## Current Gap

`codelattice_symbol`, `codelattice_change_review`, and regular `codelattice_workflow(before_edit)` can auto-route workspace roots to a selected project. The `ask` branch returns early before the normal workflow router runs, so prompts like:

```json
{
  "mode": "ask",
  "root": "/workspace",
  "language": "auto",
  "question": "如果删除 backend_target 会影响什么"
}
```

can still analyze the workspace root instead of the child project that owns `backend_target`.

## Design

1. Before calling `route_ask_intent`, detect whether the provided root is a workspace root.
2. Synthesize a routing query from explicit params or from the ask question using the existing symbol extraction path.
3. Reuse `workspace_auto_route_decision` so ask mode follows the same scoring, evidence, and large-project deferral behavior as symbol/change-review facades.
4. If routing succeeds, call `route_ask_intent` and `build_whatif_result` with the selected project root/language.
5. Preserve the original `root` in `rootRouter.originalRoot` and include `rootRouter` in the ask JSON response.
6. If routing is not confident, include a non-routed `rootRouter` hint but keep the old ask fallback behavior.

## Stop Lines

- Do not add or remove MCP tools.
- Do not execute target project code, build scripts, package managers, or tests.
- Do not modify live repositories such as `open-nwe`.
- Do not route ambiguous multi-project questions without a symbol/query/project-name signal.
- Keep compact payloads bounded.

## Verification

- Red/green test for ask before-edit using workspace root and `backend_target`.
- Existing workspace-root router tests for symbol, change_review, and workflow.
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test`
- `scripts/codelattice-installed-acceptance.sh --dev-only`
- `scripts/codelattice-precommit-check.sh`
