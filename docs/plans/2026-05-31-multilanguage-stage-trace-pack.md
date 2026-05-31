# Multi-Language Stage Trace Pack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote TypeScript, JavaScript, and Python analysis traces from coarse total-time envelopes to stage-level traces so future performance work can target the real bottleneck per language.

**Architecture:** Keep public graph output and MCP tool counts unchanged. Add `*_analysis_with_trace` wrappers beside the existing analysis functions; existing CLI callers continue using the old tuple while MCP project-once jobs consume the trace-aware tuple. Runtime capabilities should advertise `traceGranularity: "stage"` only for languages that now emit stage timing.

**Tech Stack:** Rust, serde_json trace envelopes, existing tree-sitter language adapters, MCP facade/job summary tests.

---

## Execution Card

**Write set**
- `crates/cli/src/lib.rs`
- `crates/cli/src/engine_bridge.rs`
- `crates/cli/src/mcp_facade.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-31-multilanguage-stage-trace-pack.md`

**Forbidden set**
- Do not change MCP tool counts or tool names.
- Do not alter graph schema semantics, node IDs, or edge resolution.
- Do not touch live repos such as `open-nwe` or installed `CodeLattice-Tool`.
- Do not implement parse-once extractor rewrites in this pack.

**Stop-line**
- If stage trace work requires changing TypeScript/JavaScript/Python adapter graph semantics, stop and split into a separate adapter plan.
- If tests show graph output changed, revert semantic changes and keep only observability wiring.

## Tasks

- [ ] **Task 1: Red test**
  - Modify `crates/cli/tests/mcp_server.rs::mcp_typescript_job_summary_exposes_language_runtime_trace` to expect `granularity = "stage"` and stage fields such as `projectRootMs`, `sourceDiscoveryMs`, `extractionMs`, `graphBuildMs`, and `serializationMs`.
  - Run:
    ```bash
    cargo test -p gitnexus-rust-core-cli --features tree-sitter-typescript --test mcp_server mcp_typescript_job_summary_exposes_language_runtime_trace -- --nocapture
    ```
  - Expected: fail because current trace is still `coarse`.

- [ ] **Task 2: Trace-aware analysis wrappers**
  - Add `run_typescript_analysis_with_trace`, `run_javascript_analysis_with_trace`, and `run_python_analysis_with_trace` beside existing analysis functions.
  - Existing `run_*_analysis` functions should delegate to the trace-aware version and discard trace, preserving CLI compatibility.
  - Trace schema remains `codelattice.languageAnalysisTrace.v1` with `granularity: "stage"` and `stages`.

- [ ] **Task 3: MCP project-once integration**
  - Update `crates/cli/src/engine_bridge.rs::run_project_analysis_once` to call trace-aware functions for TS/JS/Python.
  - Keep the coarse fallback for languages without detailed traces.
  - Update `crates/cli/src/mcp_facade.rs` capabilities for TS/JS/Python to `traceGranularity: "stage"`.

- [ ] **Task 4: Verification**
  - Run targeted TS test.
  - Run:
    ```bash
    cargo fmt --check
    git diff --check
    cargo test --test mcp_server
    cargo test
    scripts/codelattice-precommit-check.sh
    ```
  - If native precommit reports high/critical risk, report before committing.
