# Stage 3 — Contract Snapshot / Regression Guard Closure Review

**Date:** 2026-05-08  
**Status:** Closure Review  
**Type:** Production Graph Quality Hardening  
**Parent:** Cangjie Production Acceptance (Stage 3 of 3)

---

## Summary

Added `graph_contract.rs` — a deterministic contract regression test suite that verifies the Cangjie graph output contract for 4 fixture projects. 24 tests cover quality gates, node/edge kind sets, known symbol IDs, and known edge triples.

## Changes

### `crates/cangjie/tests/graph_contract.rs` (NEW, 24 tests on 4 fixtures)

Each fixture gets 5-8 tests covering:

| Layer | What it checks |
|-------|---------------|
| Quality gates | `imports_basic_quality_gates`, `constructor_basic_quality_gates`, `reference_cross_file_quality_gates` — 0 synthetic, 0 duplicate nodes/edges, 0 dangling, deterministic |
| Node kind set | Repository, Package, SourceFile, Symbol counts match expected |
| Edge kind set | ContainsPackage, OwnsSource, Defines, Uses, Imports counts match expected |
| Known symbols | Specific symbol IDs must exist (e.g. `sym:src/main.cj:Init:Point.init#2`) |
| Known edges | Specific edge triples must exist (e.g. `Uses sym:...main#0 → sym:...Point`) |

### Tested Fixtures

| Fixture | Tests | Key characteristics verified |
|---------|-------|---------------------------|
| `imports-basic` | 5 | Package with imports, Function/Class/Init symbols, Imports edge |
| `constructor-basic` | 6 | Multi-init class, Init #arity suffix, Uses edges from real symbols |
| `reference-cross-file-basic` | 5 | Cross-file Uses edge, Imports edge, multi-file project |
| `portable-smoke` | 8 | All Symbol kinds (Function, Class, Struct, Enum, Interface, TypeAlias, Init), cross-file + same-file Uses edges, multiple Init arities, comprehensive extraction coverage |

## Contract Coverage

### What IS covered

| Contract element | Verified by |
|-----------------|------------|
| Node kind set (Repository/Package/SourceFile/Symbol) | `*_node_kind_set` tests |
| Edge kind set (ContainsPackage/OwnsSource/Defines/Uses/Imports) | `*_edge_kind_set` tests |
| Specific symbol IDs exist | `*_known_symbols` tests |
| Specific edge triples exist | `*_known_edges` tests |
| Zero synthetic nodes (all fixtures) | `*_quality_gates` tests |
| Zero duplicate node IDs / edge triples | `*_quality_gates` tests |
| Zero dangling source/target references | `*_quality_gates` tests |
| Deterministic output (two runs = identical JSON) | `*_quality_gates` tests |
| Init symbols have #arity suffix | `constructor_basic_known_init_symbols` |
| No synthetic constructors in Uses edges | `constructor_basic_no_synthetic_constructor` |

### What is NOT covered (by design)

| Gap | Reason |
|-----|--------|
| Large production projects | Snapshots would be too large and machine-local |
| Dynamic content (diagnostics from cjc/cjlint) | Toolchain-dependent, not deterministic |
| Sort-order stability | Nodes/edges may arrive in any order; contract checks by set membership, not position |
| Full symbol ID enumeration | Only key representative symbols are checked per fixture |
| Cross-repo integration | Out of scope for Rust-core |

## Design Decisions

- **No JSON snapshots**: Contract is verified via set-membership assertions, not byte-level comparison
- **No sort-order binding**: All assertions use HashSet lookups and count-based checks, insensitive to iteration order
- **Reusable helpers**: `collect_graph()`, `assert_quality_gates()`, `assert_node_kind()`, `assert_edge_kind()`, `assert_symbol_exists()`, `assert_edge_exists()` — easily extended for new fixtures
- **Fixtures only**: No machine-local production paths; tests run everywhere with `cargo test --features tree-sitter-cangjie`

## Integrity Verification

- `cargo fmt --check`: clean
- `git diff --check`: clean
- `cargo test` (no-feature): 93 lib + all integration suites pass (contract tests properly gated)
- `cargo test --features tree-sitter-cangjie`: 112 lib + all integration suites pass
- `cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture`: 24/24 pass
- `cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`: 4/4 production targets pass

## Stop-lines compliance

- ✅ No JSON snapshots committed
- ✅ No sort-order binding
- ✅ No new dependencies
- ✅ No change to GitNexus-RC
- ✅ No change to GitNexus-RC-Tool
- ✅ No change to live repo
- ✅ No WebUI/MCP/HTTP/embedding
- ✅ No destructive git operations
- ✅ Test gated behind `tree-sitter-cangjie` feature

## How to Use

```sh
# Run contract regression tests (fixtures only, always available)
cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture

# Adding a new fixture contract:
# 1. Define a new test function following the pattern
# 2. Call collect_graph(&fixture_path("your-fixture"))
# 3. Add assert_quality_gates, assert_node_kind, assert_edge_kind...
# 4. Add assert_symbol_exists and assert_edge_exists for key IDs/edges
```

**Stage 3 status:** ✅ Complete
