# Automation Graph Pack Preflight

Date: 2026-05-18

## Goal

Add a static MCP diagnostic tool that maps repository automation entry points:
CI workflow files, package scripts, Makefile targets, Dockerfile steps, and shell scripts.

This pack is meant to help AI agents and humans answer:

- What automation exists in this repo?
- Which steps call which scripts?
- Which automation commands need review before running or changing?

## Scope

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `fixtures/automation/portable-smoke/`
- `scripts/*smoke*.sh` threshold/dogfood updates
- `README.md`, `CHANGELOG.md`, `docs/architecture/*`, `docs/plans/*`

Forbidden set:

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- CodeLattice-Tool stable runtime
- AI client configs
- live external repos

## Stop-Lines

- Static scan only.
- Do not execute scripts, CI jobs, Docker builds, package managers, or Make targets.
- Risk findings are review leads, not proof of exploitability.
- Do not claim workflow behavior is runtime verified.
- Do not add dependencies.

## Test Plan

TDD first:

1. Add automation fixture with CI, package scripts, Makefile, Dockerfile, shell scripts.
2. Add MCP tests for tool presence, summary counts, risk detection, compact output.
3. Verify red before implementation.
4. Implement scanner and MCP handler.
5. Run targeted tests, then full MCP tests and dogfood.
