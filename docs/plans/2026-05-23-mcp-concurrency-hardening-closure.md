# MCP Concurrency Hardening Closure

Date: 2026-05-23

## Summary

Implemented a session-stability guard for CodeLattice MCP concurrent tool calls. Instead of allowing multiple `tools/call` requests to race the shared process-local cache and break the stdio session, the server now runs one tool call at a time and returns a structured `mcp_server_busy` result for overlapping requests.

## Delivered

- Added `codelattice.mcpBusy.v1` response payload with retry guidance and AI-specific instructions.
- Reworked the MCP stdio loop to read requests asynchronously, run the active tool call in a worker, and keep the main loop responsive.
- Added `scripts/codelattice-mcp-concurrency-smoke.sh`.
- The smoke verifies:
  - parallel calls do not disconnect the server;
  - overlapping calls return structured busy responses;
  - the same MCP process accepts a normal call after busy responses.
- Wired the concurrency smoke into `scripts/codelattice-precommit-check.sh`.

## Boundary

This change does not make CodeLattice analysis fully parallel. That would require a deeper cache/session model. The current behavior is intentionally conservative: stable session first, explicit retry guidance second.

## Verification

- `cargo fmt --check`: pass
- `git diff --check`: pass
- `cargo build --release --all-features`: pass
- `scripts/codelattice-mcp-concurrency-smoke.sh`: pass; includes same-process recovery after busy
- `bash scripts/codelattice-mcp.sh --self-test`: pass; 49 tools, all language flags true
- `bash scripts/codelattice-mcp-ai-usability-smoke.sh`: 21/21 pass
- `bash scripts/codelattice-mcp-facade-smoke.sh`: 13/13 pass
- `cargo test --test mcp_server`: 139/139 pass
- `cargo test`: pass
- `scripts/codelattice-precommit-check.sh`: pass

Native precommit reported `critical` risk because this change touches the MCP stdio loop and native precommit script. The risk was reviewed as expected central infrastructure risk and covered by the checks above before commit.
