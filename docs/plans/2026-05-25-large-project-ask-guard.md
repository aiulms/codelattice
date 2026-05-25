# Large Project Ask Guard

## Context

Read-only smoke on `/Users/jiangxuanyang/Desktop/open-nwe/backend` showed that
`codelattice_workflow(mode=ask)` can synchronously invoke full graph analysis and
keep the MCP process busy for several minutes. This is not acceptable for the AI
facade path: `ask` should stay interactive and route large projects toward job
runtime or paged follow-up calls.

## Scope

Write set:
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`

Forbidden:
- Do not modify `open-nwe`.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not add new MCP tools.
- Do not execute target project code.

## Plan

1. Detect large ask roots through adapter file discovery.
2. For large roots, avoid synchronous full analysis in `inspect_project`,
   `before_edit`, and `locate_issue`.
3. Return compact static-only guidance with complete follow-up arguments,
   especially `mode=job`, `whatif`, `impact`, and `call_chains`.
4. Cover deferred behavior with fixture-style tests.
