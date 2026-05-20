# Analysis Scheduler / Incremental Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a small internal scheduler core that models CodeLattice analysis phases and exposes cache/staleness metadata through existing MCP cache flows.

**Architecture:** Add `gitnexus-analysis-scheduler` as an internal crate. The crate computes deterministic filesystem fingerprints and phase plans; `crates/cli/src/mcp_server.rs` remains responsible for executing analysis and storing cache entries.

**Tech Stack:** Rust 2021, serde/serde_json, existing MCP stdio tests, no new external runtime services.

---

### Task 1: Scheduler Core Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/analysis-scheduler/Cargo.toml`
- Create: `crates/analysis-scheduler/src/lib.rs`
- Create: `crates/analysis-scheduler/tests/scheduler.rs`

- [x] **Step 1: Write failing crate tests**

Add tests that assert stable phase ordering, fingerprint changes after source metadata changes, and hidden directories are ignored.

- [x] **Step 2: Run tests to verify RED**

Run: `cargo test -p gitnexus-analysis-scheduler`

Expected: package is missing or symbols are undefined.

- [x] **Step 3: Implement minimal scheduler crate**

Define `AnalysisRequest`, `AnalysisFingerprint`, `AnalysisPhase`, `AnalysisSchedule`, and `build_schedule`.

- [x] **Step 4: Run tests to verify GREEN**

Run: `cargo test -p gitnexus-analysis-scheduler`

Expected: scheduler tests pass.

### Task 2: MCP Cache Metadata Integration

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/mcp_server.rs`
- Modify: `crates/cli/tests/mcp_server.rs`

- [x] **Step 1: Write failing MCP tests**

Add tests that `codelattice_cache_status` includes `scheduler` metadata and that `codelattice_cache_prewarm` returns a `schedule` object.

- [x] **Step 2: Run targeted MCP tests to verify RED**

Run: `cargo test --test mcp_server mcp_scheduler_ -- --nocapture`

Expected: tests fail because scheduler metadata is absent.

- [x] **Step 3: Wire scheduler metadata into cache entries**

Store schedule metadata when fresh analysis runs and include it in memory cache status. Include lightweight current schedule metadata when cache is empty and root/language are provided.

- [x] **Step 4: Run targeted MCP tests to verify GREEN**

Run: `cargo test --test mcp_server mcp_scheduler_ -- --nocapture`

Expected: scheduler metadata tests pass.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/README.md`
- Modify: `docs/plans/2026-05-20-analysis-scheduler-incremental-core-closure.md`

- [x] **Step 1: Document the bottom-layer scheduler**

Add concise release-note style documentation that this is a foundation layer, not a parser/interpreter replacement.

- [x] **Step 2: Run verification**

Run required fmt, diff, scheduler tests, MCP tests, full cargo tests, all-features tests, self-test, dogfood, and detect-changes.

- [ ] **Step 3: Commit and push**

Commit with `feat(core): add analysis scheduler foundation` and push to `gitcode master`.
