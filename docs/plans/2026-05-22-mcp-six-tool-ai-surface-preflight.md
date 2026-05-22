# MCP Six-Tool AI Surface Preflight

Date: 2026-05-22

## Goal

Reduce the default external MCP surface to 6 AI-friendly entry tools while keeping all existing capabilities available through facade modes or explicit `core` / `full` toolsets.

Default `ai` tools:

- `codelattice_workflow`
- `codelattice_project`
- `codelattice_symbol`
- `codelattice_change_review`
- `codelattice_workspace`
- `codelattice_cache`

## Rationale

Some AI clients cap selected MCP tools at 40. Exposing 49 tools by default creates selection noise and can exceed client limits. CodeLattice should present a small intent-oriented surface to agents, while retaining the full diagnostic surface for debugging and regression smoke.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-mcp-facade-smoke.sh`
- `docs/guides/ai-mcp-tool-guide.md`
- `README.md`
- `CHANGELOG.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/architecture/mcp-v0-contract.md`
- `docs/plans/`

## Stop-lines

- Do not delete existing tools or handlers.
- Do not reduce `core` / `full` expert toolsets.
- Do not route workflow next actions to tools hidden from the default `ai` toolset.
- Do not claim static analysis proves runtime behavior, coverage, external use, or deletion safety.

## Verification Plan

- `cargo fmt --check`
- `git diff --check`
- focused MCP tests for default/core/root-cause workflow
- `cargo test --test mcp_server`
- `bash scripts/codelattice-mcp-facade-smoke.sh`
- `bash scripts/codelattice-precommit-check.sh`
- promote verified runtime to `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- verify promoted `codelattice-mcp.sh --self-test`

