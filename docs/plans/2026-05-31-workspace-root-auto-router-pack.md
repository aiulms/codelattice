# Workspace Root Auto Router Pack

Date: 2026-05-31

## Goal

Let AI agents pass a workspace root to symbol/change-review facades without
having to manually distinguish workspace root vs project root first.

## Why Now

Workspace jobs now analyze manifest-backed projects as project cards. The next
friction is tool entry: symbol and impact tools still expect a concrete project
root, so an agent can analyze the workspace successfully and then still make the
wrong follow-up call. This pack makes workspace root input self-correcting.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- this plan

## Forbidden Set

- Do not change MCP tool counts.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not edit live repositories such as `open-nwe`.
- Do not make workspace routing execute target code or package-manager scripts.

## Design

### 1. Detect workspace root before symbol/change analysis

When `codelattice_symbol` or `codelattice_change_review` receives a root whose
`rootDiagnosis.kind` is `workspace`, build a ranked route set from
`recommendedProjectRoots`.

### 2. Pick a concrete project root

Selection order:

1. If the query/symbol/change text clearly matches a project name/path, prefer
   that project.
2. For small projects, use the existing facade cache analyzer to inspect symbol
   names and choose the project with matching symbols.
3. For large projects, avoid probing every project. Route only when the query or
   workspace ranking is confident enough; otherwise return runnable guidance.
4. If no safe selection exists, return a non-error `needs_project_selection`
   response with runnable `recommendedNextCalls`.

### 3. Preserve evidence in output

Routed calls should include a `rootRouter` object:

- originalRoot
- selectedRoot
- selectedLanguage
- selectedProject
- confidence
- reason
- candidates

The wrapped analysis result should use the selected project root so follow-up
calls can reuse it directly.

## Stop Lines

- Stop if default AI toolset changes from 6 or full toolset changes from 49.
- Stop if a workspace root query returns silent zero results when a concrete
  project contains the symbol.
- Stop if routing probes a large project synchronously when it should defer to
  job mode.
- Stop if compact output expands full source-only lists.

## Verification

- Add a failing MCP test where `codelattice_symbol(mode=search)` receives a
  workspace root and query that exists only in a child Rust project.
- Add a failing MCP test where `codelattice_change_review(mode=impact)` receives
  the same workspace root and symbol.
- Verify the response includes `rootRouter.routed=true`.
- Verify selected root is the child project root and language is `rust`.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`,
  `cargo test`, `scripts/codelattice-installed-acceptance.sh --dev-only`, and
  `scripts/codelattice-precommit-check.sh`.
