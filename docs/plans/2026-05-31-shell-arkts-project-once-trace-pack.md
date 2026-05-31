# Shell / ArkTS Project-Once Trace Pack

Date: 2026-05-31

## Goal

Close the next MCP runtime consistency gap after C/C++: Shell and ArkTS should
also behave like project-level analyzers in facade job mode.

For AI agents, `codelattice_project(mode=job, language=shell|arkts)` should:

- submit one project-level analysis task,
- avoid synthetic per-file task cards,
- expose `codelattice.languageAnalysisTrace.v1`,
- report bounded source reads / parse passes where the analyzer can prove them.

## Current Gap

The CLI analyzers for Shell and ArkTS are already project-level. Shell is
always compiled and ArkTS is feature-gated. However, the MCP job bridge only
registers Rust, TypeScript, JavaScript, Python, C, and C++ as project-once
adapters. Shell and ArkTS job submissions therefore do not share the same
runtime contract.

## Planned Change

1. Add `run_shell_analysis_with_trace`.
2. Add `run_arkts_analysis_with_trace` behind `tree-sitter-arkts`.
3. Register `shell` and `arkts` in `engine_bridge::run_project_analysis_once`.
4. Add lightweight project-once adapters for Shell and ArkTS file discovery.
5. Mark Shell and ArkTS runtime capabilities as traceable stage-level analyzers.
6. Add MCP job tests for Shell and ArkTS proving:
   - `executor_mode = project-once`,
   - `analysisTrace.schemaVersion = codelattice.languageAnalysisTrace.v1`,
   - `parsePassesPerFile = 1`,
   - `sourceReadPasses = 1`,
   - `runtimeCapabilities.traceGranularity = stage`.

## Boundaries

- Do not change MCP tool counts.
- Do not change graph schema.
- Do not execute target project code or package manager scripts.
- Do not modify live repositories.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not include Cangjie in this slice; Cangjie needs a separate parse-once
  design because the current graph path reads/parses files across multiple
  stages and may run optional SDK diagnostics.

## Verification

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test -p gitnexus-rust-core-cli --features tree-sitter-arkts --test mcp_server mcp_arkts_project_job_trace_is_project_once -- --nocapture`
- `cargo test`
- `scripts/codelattice-precommit-check.sh`
