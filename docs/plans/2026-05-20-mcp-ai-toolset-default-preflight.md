# MCP AI Toolset Default Pack Preflight

Date: 2026-05-20

## Goal

Make CodeLattice MCP easier for AI agents to use by changing the default exposed tool surface from the full 50-tool inventory to a small AI-oriented entry layer.

## Problem

The MCP server has enough capability, but exposing 50 tools by default creates a tool-selection problem for agents. Facade tools already exist, but they only help if agents see and choose them first.

## Design

- Default `CODELATTICE_MCP_TOOLSET` becomes `ai`.
- `ai` exposes facade-first tools only, plus `codelattice_ai_context_pack`.
- `core` remains available for common low-level tools.
- `full` remains available for all tools and release/debug smoke.
- `initialize.serverInfo` reports `toolset`, visible `toolCount`, and `fullToolCount`.
- Hidden low-level tools return a structured error that points agents toward facade tools or the env var needed to expand the surface.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-mcp-facade-smoke.sh`
- `README.md`
- `CHANGELOG.md`
- `docs/plans/README.md`

## Stop-Lines

- Do not remove any existing low-level MCP tools.
- Do not break `CODELATTICE_MCP_TOOLSET=full`.
- Do not change analysis semantics.
- Do not touch GitNexus-RC, CodeLattice-Tool, AI client configs, or live repos.

## Verification

- Red/green tests for default AI toolset and hidden-tool error.
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `bash scripts/codelattice-mcp-facade-smoke.sh`
- `scripts/codelattice-precommit-check.sh`
