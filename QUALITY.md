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
