# CodeLattice Native Detect-Changes Preflight

Date: 2026-05-19

## Goal

Add a first-party `codelattice detect-changes` command so CodeLattice can replace the daily external GitNexus-Tool `detect-changes` check for its own repository and future users.

## Scope

- Add a CLI subcommand, not a new MCP tool.
- Reuse existing local MCP `codelattice_changed_symbols` and `codelattice_production_assist` behavior through the current binary's stdio MCP mode.
- Emit a stable JSON envelope for pre-commit review automation.
- Add CLI regression tests and a smoke script.
- Document usage in README and CHANGELOG.

## Non-Goals

- Do not remove or modify legacy GitNexus-Tool.
- Do not claim runtime proof, coverage proof, or compiler verification.
- Do not emulate the legacy GitNexus process model; expose `affectedProcessCount: null`.
- Do not add dependencies.
- Do not add more MCP tools.

## Risk Notes

- The change touches `crates/cli/src/lib.rs`, the main command dispatch surface.
- The implementation intentionally wraps existing MCP behavior instead of duplicating git diff to graph-symbol mapping.
- `--scope all` maps to `git diff HEAD` so staged and unstaged changes are both visible.

## Verification Plan

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands detect_changes`
- `cargo test --test productization_commands`
- `cargo test --test mcp_server`
- `cargo test`
- `scripts/codelattice-detect-changes-smoke.sh`
- `scripts/codelattice-mcp.sh --self-test`
- `scripts/mcp-dogfood.sh`
