# Facade Cache Warming Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:test-driven-development` for each bugfix/behavior change and `superpowers:verification-before-completion` before claiming completion.

**Goal:** Reduce large Rust project facade cache warming latency without regressing MCP facade usability, symbol search, impact analysis, or toolset shape.

**Architecture:** Measure the current warm path first, then optimize only the proven hot segment. The first implementation target is a Rust-only fast path that avoids unnecessary JSON/`serde_json::Value` cloning while preserving the existing MCP JSON response contract and leaving non-Rust adapters on the current compatibility path.

**Tech Stack:** Rust, CodeLattice MCP facade, `serde_json`, project-model Rust analyzer, existing `cargo test --test mcp_server` suite.

---

## Context

Related exploratory draft:

- `docs/plans/2026-05-29-facade-cache-warming-extreme-optimization.md`

That draft identifies plausible bottlenecks: JSON boundary churn, repeated filesystem walks, and serial analysis stages. Treat those as hypotheses until measured. Do not start with rayon or broad typed-graph rewrites.

Recent verified behavior:

- `codelattice_symbol(search)` can return a large-project job on cache miss.
- After job completion, facade cache can serve symbol search and `change_review impact`.
- The current remaining performance issue is facade cache warming wall-clock time on large Rust projects.

Important nuance:

- `serde_json::Value` is not reference-counted. Cloning a `Value` deeply clones the JSON tree. Existing comments or assumptions that imply cheap shared `Value` clones are wrong.

---

## Non-Goals

- Do not change the public MCP toolset count. Default AI toolset must remain 6; full toolset must remain 49.
- Do not modify `/Users/jiangxuanyang/Desktop/open-nwe` or other live repos. Only read them for smoke tests.
- Do not rewrite all language adapters.
- Do not change MCP JSON schemas unless adding backward-compatible fields.
- Do not start with rayon parallelism before deterministic output tests exist.

---

## Success Criteria

Required:

- `cargo fmt --check` passes.
- `git diff --check` passes.
- `cargo test --test mcp_server` passes.
- `scripts/codelattice-installed-acceptance.sh --dev-only` passes.
- Default toolset = 6 and full toolset = 49.
- Clean-cache open-nwe backend smoke is read-only and validates:
  - first symbol search returns either `analyzing + jobId` or a cache hit,
  - job completes successfully,
  - `facadeSymbolCount > 0`,
  - symbol search finds `preview_delegation_context_snapshot` in `src/api/delegation_context_snapshot_handlers.rs:55`,
  - `change_review impact` is not `UNKNOWN` and does not contain `Symbol not found`.

Performance target:

- Add measured `warmTrace` first.
- After optimization, report before/after values for each segment.
- Do not claim `<3s`, `<5s`, or any speedup unless fresh measurements prove it.

---

## Files Likely To Touch

- `crates/cli/src/mcp_job.rs`
  - job lifecycle, status summary, warm stage timing.
- `crates/cli/src/mcp_server.rs`
  - `McpCache`, `warm_from_result`, `build_warm_cache_entry_from_result`, `GraphView`.
- `crates/cli/tests/mcp_server.rs`
  - regression tests for warm trace, symbol/impact closure, cache digest fields.
- `scripts/codelattice-open-nwe-readonly-smoke.sh`
  - optional: strengthen clean-cache poll/retry closure.
- Possibly `crates/project-model/src/output.rs`
  - only if adding a typed Rust analysis output path after profiling proves JSON conversion is hot.

Avoid touching broad language adapters unless a test proves the change is required.

---

## Task 1: Add Warm Trace Instrumentation

**Purpose:** Turn the current performance discussion from guesses into evidence.

### Steps

- [ ] Add a small timing struct near the warm-cache code path.

Suggested fields:

```rust
#[derive(Default, Debug, Clone, serde::Serialize)]
struct WarmTrace {
    warm_total_wall_ms: u64,
    engine_result_digest_ms: u64,
    rust_analysis_ms: Option<u64>,
    analyze_value_build_ms: Option<u64>,
    graph_view_build_ms: u64,
    doc_scanner_ms: u64,
    cache_insert_ms: u64,
    scheduler_metadata_ms: Option<u64>,
    used_cli_fallback: bool,
    used_in_process_rust_analysis: bool,
}
```

- [ ] Populate it inside `warm_from_result` / `build_warm_cache_entry_from_result`.

Use `Instant::now()` around each meaningful boundary. Do not use engine task `elapsedMs` as a substitute for wall-clock warm time.

- [ ] Add `warmTrace` to job `summary` when a job reaches `succeeded`.

Keep it compact and numeric. This should be safe for AI clients.

- [ ] Add a focused MCP test.

Test shape:

```rust
#[test]
fn mcp_job_status_exposes_warm_trace() {
    let root = portable_smoke_dir();
    let mut session = McpSession::start_with_toolset("full");
    session.initialize();
    session.send_notification_initialized();

    let job = call_tool_json(
        &mut session,
        85001,
        "codelattice_project",
        serde_json::json!({
            "mode": "job",
            "root": root.to_str().unwrap(),
            "language": "rust",
            "compact": true
        }),
    );
    let job_id = job["jobId"].as_str().expect("jobId").to_string();
    let status = wait_for_job_succeeded(&mut session, 85002, "codelattice_project", &job_id);
    let trace = &status["summary"]["warmTrace"];

    assert!(trace["warm_total_wall_ms"].as_u64().is_some(), "{status:?}");
    assert!(trace["graph_view_build_ms"].as_u64().is_some(), "{status:?}");
    assert!(trace["cache_insert_ms"].as_u64().is_some(), "{status:?}");
}
```

- [ ] Run the test and verify it fails before implementation, then passes after implementation.

Commands:

```bash
cargo test --test mcp_server mcp_job_status_exposes_warm_trace -- --nocapture
```

Expected after implementation: one passing test and a job summary containing `warmTrace`.

---

## Task 2: Measure Large Rust Warm Path

**Purpose:** Record an open-nwe clean-cache baseline before changing architecture.

### Steps

- [ ] Run a dev-wrapper clean-cache smoke against `/Users/jiangxuanyang/Desktop/open-nwe/backend`.

Use a temporary `CODELATTICE_CACHE_DIR`; do not write to open-nwe.

- [ ] Capture these values in the final report:
  - total job wall-clock,
  - `progress.elapsedMs`,
  - `summary.warmTrace`,
  - `facadeSymbolCount`,
  - `facadeDigest.symbolCount`,
  - `facadeDigest.callEdgeCount`,
  - symbol search hit for `preview_delegation_context_snapshot`,
  - impact risk for the same symbol.

- [ ] If `warmTrace` shows the main cost is not JSON/GraphView, stop and update this plan before optimizing.

Stop-line:

- Do not implement Phase A until `warmTrace` proves the relevant segment is material.

---

## Task 3: Rust-Only Fast Path For Facade Graph Construction

**Purpose:** Avoid unnecessary JSON clone churn for Rust warm cache while keeping compatibility for other languages.

### Design

Keep `GraphView::build(&Value)` for all existing users. Add a Rust-only path that can build the equivalent `GraphView` from the Rust analyzer's structured output, or from a minimally cloned Rust graph representation.

Acceptable first version:

- The external MCP output remains JSON.
- Internally, `GraphView` may still expose helper methods that return `Value` for existing consumers.
- The fast path should reduce repeated full-node/full-edge clones first; it does not need a perfect final typed graph in one step.

Recommended smaller design:

```rust
struct GraphView {
    nodes: Vec<Value>,
    edges: Vec<Value>,
    nodes_by_id: HashMap<String, usize>,
    symbols_by_name: HashMap<String, Vec<usize>>,
    outgoing: HashMap<String, Vec<usize>>,
    incoming: HashMap<String, Vec<usize>>,
    diagnostics: Vec<Value>,
    language: String,
    root: String,
    doc_scanner: Option<std::sync::Arc<DocScanner>>,
}
```

This preserves current `Value`-based consumers but stores each node/edge once. It is lower risk than jumping directly to `Arc<str>` + custom enums.

### Steps

- [ ] Write a parity test for symbol search and impact using the new storage layout.

Use existing fixture roots such as `portable_smoke_dir()`.

- [ ] Refactor `GraphView` storage from duplicated `Value` clones to index-based references.

Required behavior:

- `find_symbols` still supports:
  - display name search,
  - substring search,
  - full graph id search like `symbol:crate::...`,
  - kind filter.
- `edges_from` and `edges_to` return equivalent edge values.
- `nodes_by_id` lookups remain available through a helper, not necessarily by public field access.

- [ ] Update internal callers in `mcp_server.rs`.

Search for direct uses of:

```text
gv.nodes_by_id
gv.outgoing
gv.incoming
gv.symbols_by_name
```

Convert direct map access to helper methods where needed.

- [ ] Run targeted tests.

```bash
cargo test --test mcp_server mcp_symbol_search_finds_helper -- --nocapture
cargo test --test mcp_server mcp_change_review_impact_accepts_symbol_search_id -- --nocapture
cargo test --test mcp_server mcp_symbol_search_finds_symbols_after_job_warm -- --nocapture
```

- [ ] Run full MCP tests.

```bash
cargo test --test mcp_server
```

---

## Task 4: Re-Measure And Decide Whether To Continue

**Purpose:** Avoid speculative Phase B/C work.

### Steps

- [ ] Re-run the open-nwe clean-cache benchmark from Task 2.
- [ ] Compare `warmTrace` before/after.
- [ ] If warm time is acceptable, stop here and do not implement Phase B/C.
- [ ] If filesystem walk remains a measurable bottleneck, proceed to Task 5.
- [ ] If Rust symbol/call extraction remains a measurable bottleneck, create a separate rayon plan with deterministic output tests.

---

## Task 5: Optional File Discovery Reuse

**Purpose:** Remove repeated filesystem walks only if measured.

### Constraints

Preserve existing ignore and classification behavior:

- `.git`
- `target`
- `node_modules`
- hidden directories
- tests/fixtures/script directory classification
- manifest-backed vs source-only root diagnosis

### Steps

- [ ] Add tests proving root diagnosis and source-only summaries do not change.
- [ ] Add `FileDiscovery` only in the narrow path where profiling proves repeated walk cost.
- [ ] Reuse discovered files for scheduler/fingerprint/doc scanner only if behavior remains identical.

---

## Task 6: Do Not Start Rayon Until Determinism Is Locked

Rayon is explicitly deferred. Before any parallel extraction:

- Add deterministic output tests that run the same fixture 3 times.
- Assert stable node ordering, edge ordering, diagnostic ordering, and risk ordering.
- Then parallelize one stage at a time.

Suggested command after any rayon work:

```bash
for i in 1 2 3; do
  target/debug/codelattice analyze --root fixtures/call-resolution/c1-same-module --language rust --format json > /tmp/codelattice-run-$i.json
done
diff -u /tmp/codelattice-run-1.json /tmp/codelattice-run-2.json
diff -u /tmp/codelattice-run-1.json /tmp/codelattice-run-3.json
```

---

## Required Final Verification

Run:

```bash
cargo fmt --check
git diff --check
cargo test --test mcp_server
scripts/codelattice-installed-acceptance.sh --dev-only
scripts/codelattice-precommit-check.sh
```

If the change will be installed:

```bash
scripts/codelattice-installed-acceptance.sh --sync
scripts/codelattice-installed-acceptance.sh --installed-only --require-fresh-installed
```

Final report must include:

- HEAD commit
- pushed or not
- installed or not
- `warmTrace` before/after
- open-nwe read-only proof
- toolset 6/49 proof
- any remaining warnings or risks

