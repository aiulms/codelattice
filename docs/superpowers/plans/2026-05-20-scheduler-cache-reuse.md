# Scheduler Cache Reuse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make scheduler fingerprints participate in real MCP memory and persistent cache reuse decisions.

**Architecture:** Store the scheduler fingerprint with cached entries. On lookup, rebuild the current scheduler with that previous fingerprint and return cache hits only when the scheduler decision is `reuse`.

**Tech Stack:** Rust 2021, serde_json, existing MCP stdio test harness, no new dependencies.

---

### Task 1: Red Tests for Scheduler-Driven Cache Staleness

**Files:**
- Modify: `crates/cli/tests/mcp_server.rs`

- [x] **Step 1: Add memory-cache stale test**

Create a temp Rust fixture with `Cargo.toml`, `src/lib.rs`, and `config/schema.yaml`. Analyze once, verify the second call hits memory, mutate `config/schema.yaml`, then assert the third call misses and reports scheduler fingerprint staleness.

- [x] **Step 2: Add persistent-cache stale test**

Use the same fixture shape with isolated `CODELATTICE_CACHE_DIR`. Populate cache in session 1, verify session 2 hits persistent, mutate `config/schema.yaml`, then assert session 3 misses.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test --test mcp_server mcp_scheduler_fingerprint_ -- --nocapture
```

Expected: tests fail because existing cache lookup ignores scheduler `fresh` decisions.

### Task 2: Scheduler Fingerprint Cache Metadata

**Files:**
- Modify: `crates/analysis-scheduler/src/lib.rs`
- Modify: `crates/cli/src/mcp_server.rs`

- [x] **Step 1: Expose stable scheduler helper if needed**

Add only small helpers needed by cache code. Do not change fingerprint semantics unless tests require it.

Result: no scheduler crate change was required; the MCP layer reused the existing serialized schedule fields.

- [x] **Step 2: Store memory scheduler fingerprint**

Extend `CacheEntry` with the cached scheduler fingerprint and use it on memory lookup.

- [x] **Step 3: Store persistent scheduler fingerprint**

Extend `PersistentCacheEntry` with an optional `scheduler_fingerprint` field so old cache files can fail closed or fall back safely.

- [x] **Step 4: Invalidate on scheduler mismatch**

If current schedule decision is not `reuse`, remove the cache entry and run fresh analysis. Surface `staleReason: "scheduler_fingerprint_changed"` in the fresh result metadata when the invalidation came from memory cache.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test --test mcp_server mcp_scheduler_fingerprint_ -- --nocapture
```

Expected: scheduler fingerprint cache tests pass.

### Task 3: Docs and Verification

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/README.md`
- Modify: `docs/plans/2026-05-20-scheduler-cache-reuse-closure.md`

- [x] **Step 1: Document the behavior**

Record that scheduler fingerprint now participates in cache freshness. Keep wording clear that this is static metadata, not runtime proof.

- [x] **Step 2: Run required verification**

Run fmt, diff, scheduler tests, MCP tests, full cargo tests, all-features tests, self-test, dogfood, and precommit.

- [ ] **Step 3: Commit and push**

Commit with:

```bash
git commit -m "feat(cache): reuse scheduler fingerprints"
git push gitcode master
```
