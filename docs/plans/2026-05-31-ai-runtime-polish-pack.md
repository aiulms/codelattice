# AI Runtime Polish Pack

## Goal

Make CodeLattice easier for AI agents to use after the performance/concurrency foundation work:

- Return a useful middle-size answer instead of forcing `summary` vs full graph.
- Surface dependency/framework facts without requiring the agent to open manifests manually.
- Let `ask` answer dependency/framework questions directly when possible.
- Add a normalized runtime-trace envelope so every language has the same contract, even when detailed stage timing is only available for Rust today.

## Current Context

The recent foundation work added stale baseline, delta overlay, queued jobs, compact decision cards, and faster Rust symbol extraction. CodeLattice is now usable, but AI agents still waste tokens or time in a few cases:

- CLI `analyze` has `full`, `compact`, `symbols`, `modules`, but no dependency/framework profile.
- MCP `codelattice_project(mode=quick, compact=true)` is very small, while `compact=false` can be too large. A `detail=medium` tier should include just enough structured facts.
- `codelattice_workflow(mode=ask)` handles flow/routes/issues, but dependency/framework questions fall back to generic guidance.
- Runtime capability fields exist, but there is no single trace-shaped contract that can be expanded per language over time.

## Write Set

- `crates/cli/src/ai_runtime.rs` — shared dependency/framework digest and normalized runtime trace helpers.
- `crates/cli/src/lib.rs` — add `analyze --profile deps` and expose profile docs.
- `crates/cli/src/mcp_server.rs` — add `detail=medium` project output and `ask` dependency intent.
- `crates/cli/tests/mcp_server.rs` — MCP regression tests.
- `crates/cli/tests/productization_commands.rs` — CLI profile regression test.

## Forbidden Set

- Do not modify live repositories such as `open-nwe`, `cangjie`, `warp`, or `openfang`.
- Do not change MCP tool counts or add new facade tools.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` in this pack.
- Do not execute target project code, package managers, build scripts, or tests.

## Stop Lines

- Default AI toolset must remain 6 and full toolset must remain 49.
- Compact and medium outputs must stay bounded; do not reintroduce full graph payloads.
- Dependency/framework digest is static manifest evidence only. It must not claim runtime proof.
- Runtime trace envelope may report `available=false` for languages without detailed timing; do not fabricate timing data.

## Acceptance

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test --test productization_commands`
- `scripts/codelattice-precommit-check.sh`

