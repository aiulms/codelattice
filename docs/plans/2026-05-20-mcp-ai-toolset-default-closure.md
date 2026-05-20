# MCP AI Toolset Default Pack — Closure

Date: 2026-05-20
Status: Completed

## Summary

CodeLattice MCP now defaults to an AI-friendly facade-first toolset instead of exposing the full 50-tool surface to every client.

Profiles:

- `ai` (default): 9 tools, focused on facade workflows and AI context.
- `core`: 30 tools, facades plus common low-level tools for advanced agents.
- `full`: 50 tools, complete regression/debug surface.

The full tool surface is preserved. The change only adjusts the default exposure and adds clearer guidance when a hidden tool is called.

## Behavior

- Unset or unknown `CODELATTICE_MCP_TOOLSET` maps to `ai`.
- `CODELATTICE_MCP_TOOLSET=core` exposes facade tools plus common query/review/cache/workspace tools.
- `CODELATTICE_MCP_TOOLSET=full` exposes all 50 tools.
- `initialize.serverInfo` includes:
  - `toolset`
  - visible `toolCount`
  - `fullToolCount`
  - `recommendedEntryTools`
- Hidden tool calls return structured errors:
  - `tool_not_in_ai_toolset`
  - `tool_not_in_core_toolset`

## AI Entry Guidance

Default AI clients should start with:

- `codelattice_workflow`
- `codelattice_project`
- `codelattice_symbol`
- `codelattice_change_review`
- `codelattice_workspace`
- `codelattice_release_check`
- `codelattice_cleanup`
- `codelattice_cache`

Low-level tools remain available by setting `CODELATTICE_MCP_TOOLSET=core` or `CODELATTICE_MCP_TOOLSET=full`.

## Verification

- `cargo fmt --check`: PASS
- `git diff --check`: PASS
- `cargo test --test mcp_server`: PASS, 123/123
- `bash scripts/codelattice-mcp-facade-smoke.sh`: PASS, 11/11
- `bash scripts/mcp-real-client-dry-run.sh`: PASS, 11/11
- `bash scripts/codelattice-mcp-workspace-smoke.sh`: PASS, 14/14
- `bash scripts/codelattice-mcp.sh --self-test`: PASS, 50 full tools
- `bash scripts/codelattice-precommit-check.sh`: PASS; native detect-changes reported `critical` because the change touches central MCP dispatch, release/promote/smoke scripts, and workspace graph impact fans out to supported projects plus fixture-only unsupported C# boundaries. This was reviewed as expected static-governance sensitivity rather than a failing gate.

## Script Compatibility

Release, promote, fresh-clone and full-regression smoke paths now explicitly request `CODELATTICE_MCP_TOOLSET=full` where they need low-level tools. User-facing/default MCP clients remain on the smaller AI toolset.

## Boundaries

- GitNexus-RC: not touched
- GitNexus-RC-Tool: not touched
- CodeLattice-Tool stable install: not touched
- AI client config: not touched
- Real project source: not touched
