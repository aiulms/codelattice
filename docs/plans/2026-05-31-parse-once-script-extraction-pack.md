# Parse-Once Script Extraction Pack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce script-language cold analysis overhead by removing avoidable repeated source reads and repeated parser setup in TypeScript, JavaScript, and Python project-once analyzers.

**Architecture:** Add parse-once combined extraction APIs inside the TypeScript and JavaScript adapter crates, then switch the CLI project-once analyzer to those APIs. For Python, preserve the existing two-phase semantic flow because calls need the full project function index, but cache file source text from the first pass and reuse it for call extraction.

**Tech Stack:** Rust, tree-sitter adapters, rayon project-once extraction, existing MCP stage traces.

---

## Execution Card

**Write set**
- `crates/typescript/src/extractors/*`
- `crates/javascript/src/extractors/*`
- `crates/python/src/extractors/*` only if a safe combined helper is needed
- `crates/cli/src/lib.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/plans/2026-05-31-parse-once-script-extraction-pack.md`

**Forbidden set**
- Do not change graph schema, node IDs, or edge semantics.
- Do not execute package managers, build scripts, or live project code.
- Do not modify live repos or installed `CodeLattice-Tool`.
- Do not attempt full Python parse-once call resolution in this pack; that requires a separate design because call extraction depends on project-wide symbol indexes.

**Stop-line**
- If combined extraction produces different symbols/imports/references than the existing separate extractors, stop and fix equivalence before wiring it into CLI analysis.
- If performance trace fields cannot be added without changing public graph semantics, keep them as MCP trace-only metadata.

## Tasks

- [ ] **Task 1: Red tests for adapter equivalence**
  - Add TypeScript and JavaScript tests asserting `extract_*_file()` returns the same symbols/imports/references as existing separate extractors.
  - Expected initial result: compile failure because `extract_ts_file` / `extract_js_file` do not exist yet.

- [ ] **Task 2: TypeScript parse-once helper**
  - Add root-node extraction helpers in `symbol.rs`, `imports.rs`, and `references.rs`.
  - Add `TsExtraction` and `extract_ts_file()` in `extractors/mod.rs`.
  - Existing public extractors should delegate to the root-node helpers.

- [ ] **Task 3: JavaScript parse-once helper**
  - Mirror the TypeScript structure with `JsExtraction` and `extract_js_file()`.
  - Existing public extractors should delegate to root-node helpers.

- [ ] **Task 4: CLI wiring and Python source reuse**
  - Replace TS/JS per-file triple extractor calls with the combined helpers.
  - Cache Python source text in the first extraction pass and reuse it for call extraction.
  - Add trace metadata fields such as `parsePassesPerFile` / `sourceReadPasses` so future regressions are visible.

- [ ] **Task 5: Verification**
  - Run targeted adapter tests with language features.
  - Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`, `cargo test`, and `scripts/codelattice-precommit-check.sh`.
