# Production Acceptance Preflight — Rust-native Cangjie

**Date:** 2026-05-08  
**Status:** Preflight  
**Type:** Production Acceptance Audit  
**Prior round:** Stage B Function synthetic elimination (2d5e668)

---

## 1. Current CLI Output Contract

### Commands

| Command | Input | Output | Feature gate |
|---------|-------|--------|-------------|
| `cangjie inspect --root <path>` | Cangjie project root (cjpm.toml) | JSON graph to stdout | `tree-sitter-cangjie` |
| `cangjie graph --root <path>` | Same as inspect | Identical JSON output | `tree-sitter-cangjie` |

Non-existent root → non-zero exit + stderr error message. Feature disabled → non-zero exit + "Cangjie support is disabled" error.

### Graph output shape (`CangjieGraphOutput`)

```json
{
  "nodes": [...],
  "edges": [...]
}
```

### Node types (`NodeKind`)

| Kind | ID Format | Purpose |
|------|-----------|---------|
| `Repository` | `repo:<name>` | Project root |
| `Package` | `pkg:<module-dir>/<name>` | Package from cjpm.toml |
| `SourceFile` | `src:<rel-path>` | .cj source file |
| `Symbol` | `sym:<rel-path>:<Kind>:<name>[#arity]` | Function/Class/Struct/Enum/Interface/TypeAlias/Init |
| `Diagnostic` | `diag:<file>:<line>:<col>` | cjc/cjlint diagnostic |
| `CallableSource` | `Constructor:/Method:/Function:<abs-path>:<name>#arity` | Synthetic (emitted for unresolved source IDs; currently 0 in production) |

### Edge types (`EdgeKind`)

| Kind | From | To | Purpose |
|------|------|----|---------|
| `ContainsPackage` | Repository → Package | Structural |
| `OwnsSource` | Package → SourceFile | Structural |
| `Defines` | SourceFile → Symbol | Definition |
| `Uses` | source_id → Symbol | Type annotation reference |
| `Accesses` | source_id → Symbol | Field read |
| `Modifies` | source_id → Symbol | Write/mutation |
| `Imports` | SourceFile → Package | Import dependency |
| `Annotates` | Diagnostic → Symbol | Linter/compiler annotation |

### Symbol kinds extracted

Function, Class, Struct, Enum, Interface, TypeAlias, Init, Macro (limited — `macro` function definition not supported by current grammar).

### Import resolution scope

Named imports (simple/grouped/alias), wildcard imports, package alias imports, public imports. External packages (std/core) detected and skipped (no-edge). jpm tree dependency resolution available.

### Reference extraction scope

Type annotations, function calls, constructor calls. Same-file + cross-file via import binding table. Confidence differentiated by ImportKind (ExplicitImport=0.85, PackageAlias=0.80, WildcardImport=0.70). Builtin types filtered (no Uses edge).

---

## 2. Quality Gate Coverage

### What IS covered

| Gate | Mechanism | Status |
|------|-----------|--------|
| Duplicate node IDs | `multi_project_smoke` — HashSet of node IDs | 0 on 4 targets |
| Duplicate edge triples | `multi_project_smoke` — HashSet of (kind, sourceId, targetId) | 0 on 4 targets |
| Dangling source edges | `multi_project_smoke` — source_id in node_ids | 0 on 4 targets |
| Dangling target edges | `multi_project_smoke` — target_id in node_ids | 0 on 4 targets |
| Deterministic output | `multi_project_smoke` — two runs, JSON equality | true on 4 targets |
| Synthetic nodes by kind | `multi_project_smoke` — Constructor/Method/Function breakdown | 0 on 4 targets |
| Endpoint integrity regression | `endpoint_integrity` test (12 tests) | 12/12 pass |
| Constructor synthetic elimination | `constructor_extraction` test (12 tests) | 12/12 pass |
| Alias reference resolution | `alias_reference` test (9 tests) | 9/9 pass |
| Import resolution correctness | `import_resolution` test (10 tests) | 10/10 pass |
| Cross-file reference resolution | `cross_file_reference` test (3 tests) | 3/3 pass |
| Reference extraction edge types | `reference_extraction` test (7 tests) | 7/7 pass |
| Graph output contract | `graph_parity_smoke` test (6 tests) | 6/6 pass |
| CLI integration | `cangjie_inspect` test (13 tests) | 13/13 pass |
| No-feature gracefulness | `cangjie_inspect` test (2 disabled tests) | 2/2 pass |
| Deterministic graph output | `graph_parity_smoke` → `graph_output_is_deterministic` | pass |
| Portable fixture contract | `graph_contract` → 8 tests on portable-smoke fixture | 8/8 pass |
| Portable fixture smoke | `multi_project_smoke` → `fixture_smoke_portable` | pass |

### Production smoke targets

| # | Target | Type | Availability |
|---|--------|------|-------------|
| 1 | `/Users/.../cangjie-GitNexus-Index/runtime/cjgui` | Large production project | Machine-local |
| 2 | `/Users/.../cangjie/runtime/cjgui` | Same project, different index | Machine-local |
| 3 | `/Users/.../CangjieSkills/tests/web_framework/project` | Small test project | Machine-local |
| 4 | `/Users/.../CangjieSkills/tests/json_parser/project` | Small test project | Machine-local |
| 5 | `fixtures/cangjie/portable-smoke/` | Comprehensive extraction-fixture | **Always available (repo-committed)** |

---

## 3. Quality Gate Gaps

### Not covered (by design / stop-line)

| Gap | Reason |
|-----|--------|
| Full compiler semantics | tree-sitter AST only, no type checker |
| LSP diagnostics | depends on SDK (cjc/cjlint), available but best-effort |
| jpm dependency graph | external dependency resolution via jpm tree, but not full semantic dependency graph |
| Macro expansion | grammar limitation, stop-line |
| Trait solving | stop-line (no type inference) |
| Type inference | stop-line (explicit stop-line) |
| Method dispatch | stop-line (stop-line) |
| Overload resolution | stop-line (no type information) |
| Cross-repo integration | out of scope for Rust-core (GitNexus-RC concern) |

### Not yet implemented (possible future)

| Gap | Notes |
|-----|-------|
| Operator function calls | Operator functions extracted as symbols, but operator call references not handled |
| Generic function instantiations | Generic definitions extracted, but instantiation references not handled |
| Field access deep resolution | Accesses/Modifies edges exist for simple field access, but not deeply resolved |
| VArray/Collection literal type refs | Not yet extracted |

---

## 4. Stop-lines (reaffirmed)

- No MCP server
- No WebUI
- No GitNexus-RC bridge / cross-repo glue
- No default tool replacement (GitNexus-RC remains primary)
- No live repo writes
- No HTTP API
- No embeddings
- No new dependencies without gate document
- No destructive git operations
- No .codeartsdoer/ or temporary directory commits
- No type inference / trait solving
- No macro expansion
- No method dispatch
- No overload resolution

---

## 5. Running Acceptance Tests

### Quick check (fixtures only, always available)

```sh
# Contract regression — 24 tests on 4 fixtures, < 0.1s
cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture

# Multi-project smoke — 4 fixture tests, < 0.1s
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --nocapture

# Both together
cargo test --features tree-sitter-cangjie --test graph_contract --test multi_project_smoke -- --nocapture
```

### Full acceptance suite (fixtures + production paths)

```sh
# Includes 4 machine-local production targets (~30s)
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
```

Production targets are `#[ignore]`-guarded — missing paths are gracefully skipped, not hard failures.

### Full verification sequence (for commits)

```sh
cargo fmt --check
git diff --check
cargo test                                    # no-feature: 93 lib + integration
cargo test --features tree-sitter-cangjie     # feature: 112 lib + all integration suites
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
```

### Interpreting results

| Signal | Meaning |
|--------|---------|
| `PASS` + synth=0, dup=0, dang=(0,0), det=true | Quality gate green |
| `SKIP` + reason | Target not available (production path missing, no cjpm.toml) |
| `FAIL` + reason | Quality gate violation — requires investigation |
| Summary `fail: 0` | All tested targets pass acceptance criteria |

---

## 6. Production Acceptance Judgment

**Current state: READY for local trial use as a development-quality graph tool.**

Evidence:
- 4 real-world Cangjie projects produce clean, complete graphs
- All quality gates pass (0 dup, 0 dangling, deterministic, 0 synthetic)
- 9 fixture-based integration test suites cover core extraction/resolution/graph paths
- CLI contract is simple and well-defined (one-shot JSON output)
- Feature gate works correctly (graceful disable)
- Zero Rust warnings in both builds

Not ready for:
- Production replacement of GitNexus-RC (governance stop-line, not a quality issue)
- Automated CI/CD without human review (production paths are machine-local)
- User-facing product (no error recovery, no incremental analysis, no caching)

### Recommended next phase

After Stage 2 (smoke ergonomics) and Stage 3 (contract regression guard):
- ✅ Added `--strict` mode that fails on any synthetic > 0 (952f326)
- ✅ Added dedicated CLI tests + docs for `--strict` (strict quality gate follow-up)
- ✅ Added portable production fixtures (d048bf9, `fixtures/cangjie/portable-smoke/`)
- ✅ Documented acceptance criteria as standalone QUALITY.md
