# Cangjie Graph Quality Gates

**Last updated:** 2026-05-08
**Status:** Active
**Source:** [Production Acceptance Preflight](docs/plans/2026-05-08-cangjie-production-acceptance-preflight.md)

This document defines the quality acceptance criteria for the Rust-native Cangjie graph output. It is the single source of truth for what "production-quality" means and how to verify it.

---

## Quality Gates

Every Cangjie graph output must satisfy these invariants:

| Gate | Threshold | Verified by |
|------|-----------|-------------|
| Duplicate node IDs | 0 | `multi_project_smoke`, `graph_contract` |
| Duplicate edge triples | 0 | `multi_project_smoke`, `graph_contract` |
| Dangling source references | 0 | `multi_project_smoke`, `graph_contract` |
| Dangling target references | 0 | `multi_project_smoke`, `graph_contract` |
| Deterministic output | Two runs produce identical JSON | `multi_project_smoke`, `graph_contract` |
| Synthetic nodes (CallableSource) | 0 | `multi_project_smoke`, `graph_contract` |
| Init symbols have `#arity` suffix | All Init symbols match | `graph_contract`, `multi_project_smoke` |
| No-feature build has zero Rust warnings | 0 (excluding vendored scanner.c) | `cargo test` |

## Contract Regression Gates

The `graph_contract` test suite (24 tests on 4 fixtures) additionally verifies:

| Contract element | How verified |
|-----------------|-------------|
| Node kind set present | Repository, Package, SourceFile, Symbol counts per fixture |
| Edge kind set present | ContainsPackage, OwnsSource, Defines, Uses, Imports counts per fixture |
| Known symbol IDs exist | Specific Symbol node IDs present in graph |
| Known edge triples exist | Specific (kind, sourceId, targetId) present in graph |

### Contract Fixtures

| Fixture | Tests | What it exercises |
|---------|-------|-------------------|
| `imports-basic` | 5 | Named/grouped/wildcard/alias imports, Function/Class/Init symbols |
| `constructor-basic` | 6 | Multi-init class, Init `#arity` suffix, Uses edges from real symbols |
| `reference-cross-file-basic` | 5 | Cross-file Uses edge, Imports edge, multi-file project |
| `portable-smoke` | 8 | **All** Symbol kinds (Function, Class, Struct, Enum, Interface, TypeAlias, Init), cross-file + same-file Uses, Imports, multiple Init arities |

## Running Acceptance Tests

### Quick check (fixtures only, always available, < 0.1s)

```sh
# Contract regression — 24 tests on 4 fixtures
cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture

# Multi-project smoke — 4 fixture tests
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --nocapture

# Both together
cargo test --features tree-sitter-cangjie --test graph_contract --test multi_project_smoke -- --nocapture
```

### Full acceptance suite (fixtures + machine-local production paths, ~30s)

```sh
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
```

### Full verification sequence (for commits)

```sh
cargo fmt --check
git diff --check
cargo test                                    # no-feature
cargo test --features tree-sitter-cangjie     # feature-enabled
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
```

## Interpreting Results

| Signal | Meaning |
|--------|---------|
| `PASS` + `synth=0, dup=0, dang=(0,0), det=true` | Quality gate green — no action needed |
| `SKIP` + reason | Target not available (missing path, no cjpm.toml) — not a failure |
| `FAIL` + reason | Quality gate violation — **must investigate before proceeding** |
| Summary `fail: 0` | All tested targets pass acceptance criteria |

## Adding a New Contract Fixture

1. Create fixture directory under `fixtures/cangjie/<name>/` with `cjpm.toml` and `.cj` sources
2. In `graph_contract.rs`, add test functions following the pattern:
   - `collect_graph(&fixture_path("your-fixture"))` to collect data
   - `assert_quality_gates(&data)` for invariants
   - `assert_node_kind()` / `assert_edge_kind()` for kind coverage
   - `assert_symbol_exists()` / `assert_edge_exists()` for known IDs/edges
3. In `multi_project_smoke.rs`, add a `fixture_smoke_<name>` test
4. Run full verification sequence above

## `--strict` Flag

Both `cangjie inspect` and `cangjie graph` accept a `--strict` flag (default: `false`):

```sh
cangjie inspect --root <path> --strict
cangjie graph --root <path> --strict
```

**Behavior:**
- With `--strict`, the CLI counts `CallableSource` (synthetic) nodes after graph emission
- If synthetic > 0, the CLI exits non-zero with an error message on stderr
- If synthetic = 0, output is identical to non-strict mode
- Feature-disabled builds accept `--strict` without error (graceful no-op, same as non-strict disabled path)

**Purpose:** Enforce the zero-synthetic quality gate at the CLI level for CI/CD or scripting, without requiring human inspection of smoke test output.

**Limitations:**
- `--strict` only checks synthetic > 0; it does not verify duplicate node IDs, dangling edges, or determinism (those remain covered by test suites)
- No fixture currently triggers synthetic > 0 in production builds (all current fixtures and production targets produce 0 synthetic)
- Strict failure (synthetic > 0 → non-zero exit) is tested indirectly by the `multi_project_smoke` and `graph_contract` quality gate suites, which hard-assert `synthetic_count = 0`

**Tests:** `crates/cli/tests/cangjie_inspect.rs` — 8 dedicated tests covering strict success (valid JSON, graph parity), strict + nonexistent root, and strict + feature-disabled.

## Known Gaps (by design)

| Gap | Reason |
|-----|--------|
| Full compiler semantics | tree-sitter AST only, no type checker |
| LSP diagnostics | SDK-dependent (cjc/cjlint), best-effort |
| Macro expansion | Grammar limitation — stop-line |
| Type inference / trait solving | Explicit stop-line |
| Method dispatch / overload resolution | Stop-line (no type information) |
| Sort-order stability | Contract checks by set membership, not position |

## Stop-lines

These boundaries are non-negotiable for Rust-core Cangjie:

- No MCP server, WebUI, HTTP API, embeddings
- No GitNexus-RC bridge / cross-repo glue
- No default tool replacement (GitNexus-RC remains primary)
- No live repo writes
- No new dependencies without gate document
- No destructive git operations
- No type inference / trait solving / macro expansion / method dispatch / overload resolution
- No `.codeartsdoer/` or temporary directory commits

---

# Rust Graph Quality Gates

**Last updated:** 2026-05-08（Slice 53: enum variant extraction + classification fix）
**Status:** Active
**Source:** [Rust Production Readiness Smoke Audit](docs/plans/2026-05-08-rust-production-readiness-preflight.md)

This section defines the quality acceptance criteria for the Rust graph output. It mirrors the Cangjie quality gate structure above.

---

## Quality Gates

Every Rust graph output must satisfy these invariants:

| Gate | Threshold | Verified by |
|------|-----------|-------------|
| Duplicate node IDs | 0 | `project_model_graph_contract` |
| Duplicate edge triples | 0 | `project_model_graph_contract` |
| Dangling source references | 0 | `project_model_graph_contract` |
| Dangling target references | 0 | `project_model_graph_contract` |
| Deterministic output | Two runs produce identical JSON | `project_model_graph_contract` |
| External symbol nodes have `isExternal: true` | All external nodes marked | `graph_contract` (imports-cross-crate) |
| CALLS endpoint integrity | All CALLS source/target exist in nodes | `project_model_graph_contract` |

## Contract Regression Gates

The `project_model_graph_contract` test suite (44 tests on 6 fixtures) verifies:

| Contract element | How verified |
|-----------------|-------------|
| Node kind set present | Repository, Package, Target, SourceFile, Symbol counts per fixture |
| Edge kind set present | CONTAINS_PACKAGE, HAS_TARGET, OWNS_SOURCE, DEFINES, CALLS, DESIGNATION, ACCESSES |
| Known symbol IDs exist | Specific Symbol node IDs present in graph |
| Known edge triples exist | Specific (kind, source, target) present in graph |
| CALLS endpoint integrity | Every CALLS edge source/target exists as node |
| External symbol node marking | stdlib target symbols have isExternal=true |

### Contract Fixtures

| Fixture | Tests | What it exercises |
|---------|-------|-------------------|
| `portable-smoke` | 8 | Repository/Package/Target/SourceFile nodes, DEFINES/CALLS/DESIGNATION/ACCESSES edges, cross-target calls |
| `imports-cross-crate` | 8 | External symbol nodes (4 stdlib types), external crate CALLS, ACCESSES edges for same-crate types, DESIGNATION |
| `multi-module` | 7 | Multi-file project, crate:: path CALLS, cross-file DEFINES, multiple OWNS_SOURCE |
| `module-hierarchy` | 7 | Multi-level module tree, crate:: direct path, super:: path, import-resolved CALLS, cross-file DEFINES |
| `inline-module` | 7 | Inline modules with nested definitions, crate:: path from nested module, HAS_PARENT edges for module hierarchy |
| `self-path` | 7 | self:: path resolution for free functions, self:: path to associated function, module hierarchy HAS_PARENT, DESIGNATION edges |

## Running Acceptance Tests

```sh
# Rust graph contract regression — 44 tests on 6 fixtures
cargo test --test project_model_graph_contract -- --nocapture

# Full no-feature test suite
cargo test

# Full feature-enabled test suite
cargo test --features tree-sitter-cangjie
```

## Current Production Stats (gitnexus-rust-core self-smoke)

| Metric | Value |
|--------|-------|
| Packages | 3 (gitnexus-project-model, gitnexus-rust-core-cli, gitnexus-cangjie) |
| Source files | 50 |
| Symbols | 783 (incl. 173 enum variants) |
| Imports | variable |
| Total calls | 3,608 |
| Resolved calls | 2,369 (65.7%) |
| Graph nodes | — (run `cargo run -- project-model graph --root .` to refresh) |
| Graph edges | — (run `cargo run -- project-model graph --root .` to refresh) |
| CALLS edges | — (run `cargo run -- project-model graph --root .` to refresh) |
| Duplicate nodes | 0 |
| Duplicate edges | 0 |
| Dangling sources | 0 |
| Dangling targets | 0 |
| Deterministic | yes |

### Resolved Call Distribution

| Reason | Count | % of resolved |
|--------|-------|---------------|
| stdlib-trait-method-resolved | 914 | 39.1% |
| same-module-resolved | 474 | 20.3% |
| known-enum-constructor | 313 | 13.4% |
| receiver-type-method-resolved | 267 | 11.4% |
| external-crate-path-resolved | 207 | 8.9% |
| same-file-unique-name | 67 | 2.9% |
| method-name-resolved | 35 | 1.5% |
| same-crate-resolved (Phase 2e+2f) | 23 | 1.0% |
| module-path-resolved | 18 | 0.8% |
| import-resolved | 10 | 0.4% |
| associated-fn-resolved (Phase 2g+Slice48+Slice50) | 7 | 0.30% |

## Known Gaps (by design)

| Gap | Reason |
|-----|--------|
| 1,219 unresolved calls (34.1%) | 1,181 method-calls need type inference (stop-line); 16 free-function; 10 associated-function; 7 qualified-path; 5 external-crate |
| Method dispatch limited | No type inference / trait solving (stop-line) |
| Wildcard import not expanded | Stop-line: no macro expansion |
| ACCESSES edges — same crate only | External type nodes only created for CALLS targets |
| No macro expansion | `foo!()` calls not expanded (stop-line) |
| No cfg evaluator | cfg-gated `mod` marked unknown (stop-line) |
| No `cargo metadata` | Manifest-derived project model only (stop-line) |

## Stop-lines

These boundaries are non-negotiable for Rust-core Rust:

- No type inference / trait solving
- No macro expansion
- No full cfg evaluator
- No `cargo metadata` execution
- No proc-macro / build.rs execution
- No arbitrary external crate API symbol resolution (std/core/alloc direct path only)
- No MCP server, WebUI, HTTP API, embeddings
- No production replacement for GitNexus-RC
- No live repo modification
- No GitNexus-RC runtime/schema modification
- No `.codeartsdoer/` or temporary directory commits
- No new dependencies without gate document
- No destructive git operations
