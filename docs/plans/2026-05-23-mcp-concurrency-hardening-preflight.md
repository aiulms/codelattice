# MCP Concurrency Hardening Preflight

Date: 2026-05-23

## Problem

Real AI clients can issue multiple CodeLattice MCP `tools/call` requests in parallel. On large roots such as `open-nwe`, that made the MCP stdio session disconnect or become unavailable until the client restarted. Language support flags and wrapper self-test are already fixed; the remaining blocker is session stability under concurrent calls.

## Goal

Keep CodeLattice MCP stable for AI users even when the client sends parallel tool calls. The server must not disconnect. It may reject concurrent calls with a structured, retryable response because CodeLattice analysis uses shared process-local cache state.

## Execution Card

- Write set: `crates/cli/src/mcp_server.rs`, `scripts/`, `docs/plans/`, `CHANGELOG.md`.
- Forbidden set: live repos (`open-nwe`, `cangjie`), AI client configuration, GitNexus-RC, CodeLattice-Tool source.
- Stop line: do not change parser semantics or analysis output contracts beyond the new busy response.

## Intended Behavior

- First `tools/call` in a session may run normally.
- While it is running, additional `tools/call` requests receive `codelattice.mcpBusy.v1` with `mcp_server_busy`.
- After the running call finishes, the same MCP process accepts and answers later calls.
- AI guidance tells clients not to parallelize CodeLattice calls and to prefer `codelattice_workflow execute=true` for orchestration.

## Verification Plan

- Add an MCP concurrency smoke that sends multiple `tools/call` requests in one stdio session.
- Assert at least one structured busy response, no JSON-RPC transport error, and same-process recovery after busy.
- Wire the smoke into native precommit.
- Run MCP regression, facade smoke, AI usability smoke, wrapper self-test, and native precommit before commit.
