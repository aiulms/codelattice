# AI Facade Ergonomics Pack

Date: 2026-06-01

## Goal

Make the six-tool MCP facade easier for AI agents to use without wasting context:

- Default AI facade responses should be compact unless a caller explicitly asks for `compact=false`.
- Compact responses must not re-expand full `rootDiagnosis.sourceOnlyEntries`.
- Ambiguous symbol responses should provide runnable follow-up calls with exact symbol ids, and may safely select the only production candidate when other matches are tests/fixtures.
- Workspace roots should route or guide toward a concrete project root instead of forcing trial-and-error.
- The AI usage guide should encode the preferred workflow so clients do not rediscover it by testing.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/mcp/ai-usage-guide.md`

## Forbidden Set

- Do not modify live repositories such as `open-nwe`.
- Do not change tool counts or add a new facade tool.
- Do not remove static-analysis cautions or pretend compact output is runtime proof.

## Stop Line

Stop and report before committing if:

- Default AI toolset is no longer 6 or full toolset is no longer 49.
- `compact=true` regains full `sourceOnlyEntries` payloads.
- Ambiguous symbol handling silently chooses between multiple production candidates.
- Native precommit reports high/critical risk.
