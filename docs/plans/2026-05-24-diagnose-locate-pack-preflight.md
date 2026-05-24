# Diagnose / Locate Pack — Preflight

## Goal

Make CodeLattice more useful for AI agents that are trying to answer:

- where is this bug or symptom likely located?
- what should I read first?
- which entry points or impact boundaries matter before editing?

This pack keeps the public MCP surface small by adding a `diagnose` mode to the existing `codelattice_project` facade and a `diagnose_issue` intent to `codelattice_workflow`.

## Scope

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/`
- `CHANGELOG.md`

Forbidden set:

- Do not sync or promote to `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not touch GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool, AI client config, or live repo source.
- Do not add new MCP tools.
- Do not execute target project code.

## Design

`codelattice_project(mode=diagnose)` will run static graph analysis and return:

- `likelyAreas`: ranked files/symbols/modules likely related to the symptom/query/path.
- `readFirst`: concrete file/symbol reading order.
- `entryPoints`: relevant architecture entries from insights.
- `impactHints`: callers/callees/cross-file static hints.
- `confidence`, `reason`, `nextAction` on each item.
- `queryTerms` and `inputSignals` so AI agents know what was matched.

`codelattice_workflow(mode=diagnose_issue)` will act as an AI-friendly router:

- missing symptom/query/error text produces a `missingInputs` item instead of failure.
- valid input points to `codelattice_project(mode=diagnose)`.
- `execute=true` may run the diagnose action through existing workflow execution.

## Stop-lines

- Static diagnosis is only a hypothesis. It must not claim root-cause proof.
- Runtime/test/build evidence remains outside CodeLattice unless explicitly run by the user/agent.
- No parallel MCP call behavior changes in this pack.

## Verification Plan

- Add failing MCP tests before implementation.
- Run focused tests for diagnose/project workflow.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`.
- Run native `detect-changes` before commit.
