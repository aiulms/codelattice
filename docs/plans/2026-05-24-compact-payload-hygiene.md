# Compact Payload Hygiene Pack

Date: 2026-05-24

## Goal

Make `compact=true` MCP facade responses easier for AI agents to read by removing large source-only directory lists from `rootDiagnosis`.

## Scope

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-installed-acceptance.sh`
- `scripts/promote-to-local-tool.sh`
- `docs/mcp/ai-usage-guide.md`
- `CHANGELOG.md`

Forbidden set:

- Do not modify live repositories such as `open-nwe`.
- Do not reduce the default AI toolset.
- Do not remove full diagnostics from `compact=false`.

## Design

For `compact=true`, `rootDiagnosis` should keep root kind, counts, recommended project roots, cautions, and `sourceOnlySummary`, but omit full `sourceOnlyEntries`.

When a small sample helps the AI choose a next root, expose `sourceOnlyEntryPreview` with at most five minimal entries. Full source-only diagnostics remain available through `compact=false` or workspace/project detail paths.

## Verification

- Add MCP regression tests proving compact root diagnosis omits `sourceOnlyEntries`.
- Keep full-mode source-only diagnostics intact.
- Extend installed acceptance smoke with a payload-size/shape check.
- Keep installed sync/promotion on the full-language runtime and verify the promoted wrapper after sync.
- Run formatting, MCP tests, native precommit, installed sync, and strict installed acceptance.
