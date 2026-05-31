# Cangjie Project-Once Runtime Closure Pack

Date: 2026-05-31

## Goal

Close the last multi-language MCP runtime consistency gap: Cangjie project jobs
should use the same project-once job contract and normalized stage trace as
Rust, TypeScript, JavaScript, Python, C, C++, Shell, and ArkTS.

For AI agents, `codelattice_project(mode=job, language=cangjie)` should:

- submit one project-level analysis task,
- return `executor_mode = project-once`,
- expose `codelattice.languageAnalysisTrace.v1`,
- report bounded source reads and parse passes,
- stay static-only and avoid optional SDK diagnostics in the MCP job path.

## Current Gap

Cangjie CLI analysis exists and can emit graphs, but MCP project job runtime is
not registered as a project-once analyzer. Facade runtime capabilities also
report no trace for Cangjie. This makes Cangjie the only supported language left
outside the normalized project-once runtime surface.

The existing Cangjie graph helper may run optional SDK diagnostics when an SDK is
available. That is useful for explicit CLI inspection, but not for the MCP AI
runtime contract, where tool output must remain static-only and must not invoke
external compilers or linters during project job analysis.

## Planned Change

1. Add `run_cangjie_analysis_with_trace` behind `tree-sitter-cangjie`.
2. Implement its MCP job path as static-only:
   - build project model,
   - read each `.cj` source once,
   - parse each source once,
   - extract symbols/imports/references from the stored source + tree,
   - emit graph nodes/edges using existing Cangjie graph emitters,
   - do not call `run_all_diagnostics`.
3. Add a disabled-feature stub so source builds without Cangjie still fail
   gracefully.
4. Register Cangjie in `engine_bridge::run_project_analysis_once` and
   `get_adapter_for_language`.
5. Mark Cangjie runtime capabilities as stage-traceable only when the feature is
   enabled.
6. Add an MCP job regression test proving the project-once trace contract.

## Boundaries

- Do not change MCP tool counts.
- Do not change Cangjie graph schema.
- Do not alter the explicit `cangjie inspect/graph` CLI path.
- Do not execute target code, build scripts, package managers, cjc, or cjlint in
  the MCP project job path.
- Do not modify live repositories.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.

## Verification

- `cargo test -p gitnexus-rust-core-cli --features tree-sitter-cangjie --test mcp_server mcp_cangjie_project_job_trace_is_project_once -- --nocapture`
- `cargo test -p gitnexus-rust-core-cli --features tree-sitter-cangjie --test productization_commands analyze_cangjie_explicit_language -- --nocapture`
- `cargo fmt --check`
- `git diff --check`
- `cargo check -p gitnexus-rust-core-cli --features tree-sitter-cangjie`
- `cargo test --test mcp_server`
- `cargo test`
- `scripts/codelattice-precommit-check.sh`
