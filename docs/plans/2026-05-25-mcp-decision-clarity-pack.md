# MCP Decision Clarity Pack

Date: 2026-05-25

## Goal

Reduce ambiguity in AI-facing CodeLattice MCP facade output without changing the six-tool default surface or syncing the installed `CodeLattice-Tool` runtime yet.

## Execution Card

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/mcp/ai-usage-guide.md`
- `CHANGELOG.md`

Forbidden set:

- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not modify live repositories such as `open-nwe`.
- Do not add a new MCP tool.
- Do not treat static analysis as runtime or coverage proof.

Stop line:

- Stop if the default AI toolset changes from six facade tools or the full toolset changes from 49 tools.
- Stop if compact responses regain large `sourceOnlyEntries` payloads.

## Design

Add decision metadata to existing facade responses instead of adding tools:

1. `decisionGuidance` tells an AI which facade/mode/root type the response belongs to, when to use the current tool, and what to call next.
2. `rootDiagnosis` clarifies whether the input is a workspace root or a project root, and explicitly marks source-only entries as focused drill-down candidates rather than manifest-backed projects.
3. Risk outputs gain `priorityRank`, `relativePriority`, `riskDrivers`, and `riskScoreInterpretation` so "all high" lists remain sortable.
4. Compact responses report what was omitted and where to fetch full detail.

## Verification

- Add failing MCP regression tests first.
- Run targeted MCP tests.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`.
- Run native precommit before committing.
