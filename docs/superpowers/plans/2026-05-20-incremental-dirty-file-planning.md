# Incremental Dirty-file Planning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add scheduler-owned dirty-file planning metadata for stale cache paths.

**Architecture:** Extend the scheduler with file snapshots and an `incrementalPlan`, then have MCP store snapshots in memory/persistent cache entries and pass them back into future schedules. MCP still executes full analysis on miss.

**Tech Stack:** Rust 2021, serde, serde_json, existing MCP stdio test harness, no new dependencies.

---

### Task 1: Scheduler Dirty-file Plan

**Files:**
- Modify: `crates/analysis-scheduler/src/lib.rs`
- Modify: `crates/analysis-scheduler/tests/scheduler.rs`

- [x] **Step 1: Write failing scheduler test**

Add a test that builds a schedule, mutates `src/lib.rs`, passes the old file snapshot into a second schedule, and expects `incrementalPlan.dirtyFiles[0].path == "src/lib.rs"`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p gitnexus-analysis-scheduler dirty_file_plan -- --nocapture
```

Expected: fail because no previous file snapshot or incremental plan exists.

- [x] **Step 3: Implement scheduler snapshot and plan**

Add public `FileSnapshot`, `DirtyFile`, and `IncrementalPlan` types. Compare previous and current snapshots by relative path and classify added, removed, and modified files.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p gitnexus-analysis-scheduler dirty_file_plan -- --nocapture
```

Expected: pass.

### Task 2: MCP Cache Metadata

**Files:**
- Modify: `crates/cli/src/mcp_server.rs`
- Modify: `crates/cli/tests/mcp_server.rs`

- [x] **Step 1: Write failing MCP test**

Add a stale-cache test that mutates `config/schema.yaml` and expects `schedule.incrementalPlan` to include that file and a full-analysis strategy.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test mcp_server mcp_scheduler_incremental_plan_ -- --nocapture
```

Expected: fail because MCP does not store/pass scheduler file snapshots yet.

- [x] **Step 3: Store previous file snapshots**

Persist the scheduler file snapshot in memory and disk cache entries, pass it into `build_scheduler_metadata`, and keep the serialized schedule response compact.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test --test mcp_server mcp_scheduler_incremental_plan_ -- --nocapture
```

Expected: pass.

### Task 3: Docs and Verification

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/README.md`
- Modify: `docs/plans/2026-05-20-incremental-dirty-file-planning-closure.md`

- [x] **Step 1: Document behavior**

Record that dirty-file planning is metadata only and does not yet execute partial graph rebuilds.

- [x] **Step 2: Run verification**

Run scheduler tests, MCP tests, full cargo tests, all-features tests, MCP self-test, dogfood, and native precommit.

- [ ] **Step 3: Commit and push**

Commit with:

```bash
git commit -m "feat(scheduler): plan dirty file analysis"
git push gitcode master
```
