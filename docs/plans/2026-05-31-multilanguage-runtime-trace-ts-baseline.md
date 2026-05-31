# Multi-Language Runtime Trace And TS Baseline Plan

> Execution note: keep this pack focused on the AI runtime substrate. Do not add new MCP tools, do not sync the installed CodeLattice-Tool, and do not modify live repositories.

## Goal

Make CodeLattice tell AI agents how each language analysis actually ran, so performance and concurrency work is driven by evidence instead of anecdotes. Rust already exposes detailed `AnalysisTrace`; this pack adds a normalized coarse trace for TypeScript, JavaScript, and Python project-once jobs, then surfaces it through MCP runtime envelopes and capabilities.

## Current State

- Rust project-once analysis returns a detailed project-model `AnalysisTrace`.
- TypeScript, JavaScript, and Python project-once analysis return `analysis_trace: None`, even though job summaries and warm cache already have a slot for `analysisTrace`.
- `runtimeCapabilities.traceAvailable` is currently `false` for non-Rust languages, so AI agents cannot tell whether a slow TS job is missing instrumentation or genuinely slow.
- Warm cache timing still has a Rust-specific field name (`rustAnalysisMs`) even when the job is not Rust, which can confuse downstream readers.

## Design

1. Add a normalized trace schema for non-Rust languages:
   - `schemaVersion: codelattice.languageAnalysisTrace.v1`
   - `language`
   - `granularity: coarse`
   - `totalMs`
   - `sourceFileCount`, `symbolCount`, `callEdgeCount`, `nodeCount`, `edgeCount`
   - `stages.projectAnalysisMs`
   - static-analysis semantics (`targetCodeExecuted=false`)
2. Keep Rust detailed trace unchanged for compatibility, but teach the runtime envelope to label Rust as `granularity=detailed`.
3. Surface trace availability in `facade_language_runtime_capabilities` for all project-once languages that now emit traces.
4. Add backward-compatible warm trace fields:
   - Keep `rustAnalysisMs` for old consumers.
   - Add `languageAnalysisMs` and `language` for accurate multi-language interpretation.
5. Reduce a shared slow path for TypeScript, JavaScript, and Python by parallelizing per-file extraction with deterministic `BTreeMap` reassembly. Each file already creates its own parser/extractor state, so file-level parallelism does not change resolver semantics.

## Write Set

- `crates/cli/src/engine_bridge.rs`
- `crates/cli/src/ai_runtime.rs`
- `crates/cli/src/mcp_facade.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-31-multilanguage-runtime-trace-ts-baseline.md`

## Forbidden Set

- `/Users/jiangxuanyang/Desktop/open-nwe`
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- GitNexus-RC runtime/schema
- MCP tool count changes

## Stop Lines

- Do not introduce TypeScript semantic changes while adding trace. If TS graph counts change, stop and investigate.
- Do not add heavy per-file tracing payloads to compact responses.
- Do not make `runtimeTrace.available=true` unless a real trace object exists in the current result.

## Acceptance

- Default AI toolset remains 6 and full toolset remains 49.
- TypeScript project-once job with `wait=true` returns `summary.analysisTrace.schemaVersion == codelattice.languageAnalysisTrace.v1`.
- TypeScript runtime capabilities report `traceAvailable=true` and `traceGranularity=coarse`.
- TypeScript and JavaScript/Python per-file extraction can run in parallel while preserving sorted graph assembly.
- Compact facade runtime trace can consume the coarse trace without returning bulky payloads.
- Existing Rust trace behavior remains compatible.
- Verification: `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`, `cargo test`, and `scripts/codelattice-precommit-check.sh`.
