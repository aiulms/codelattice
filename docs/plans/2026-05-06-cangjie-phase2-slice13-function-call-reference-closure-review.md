# Cangjie Phase 2 Slice 13 — Function Call Reference Extraction Closure Review

**Date:** 2026-05-06
**Type:** closure-review（docs-only）
**Status:** Complete
**Author:** aiulms

## 1. What was delivered

Slice 13 added function call reference extraction to the Cangjie references extractor.
Function calls and constructor calls now produce USES edges, closing the gap where
`postfixExpression` with `callSuffix` was explicitly skipped.

### 1.1 Supported call forms

| Call form | Example | Edge | Confidence |
|-----------|---------|------|-----------|
| Simple function call | `add(1, 2)` | USES | 0.80 |
| Constructor call | `Point(0, 0)` | USES | 0.80 |
| Qualified call | `pkg.func(args)` | USES | 0.80 |
| Cross-file (via explicit import) | `import mathpkg.ops.{add}` → `add(1,2)` | USES | 0.85 |

### 1.2 Unsupported (per stop-lines)

| Call form | Reason |
|-----------|--------|
| Method call (`obj.method()`) | Requires receiver type inference |
| Wildcard import call | Requires deep expansion |
| Alias-renamed import call | Requires alias tracking |
| External dependency call | Requires version/git resolution |

## 2. Changes

### 2.1 Modified files

- **`crates/cangjie/src/extractors/references.rs`** — Added `has_call_suffix()`,
  `extract_callee_name()` helpers, and function call handler in `walk()` (~75 lines)

### 2.2 New files

- **`crates/cangjie/tests/function_call_reference.rs`** — 10 integration tests
- **`fixtures/cangjie/reference-function-call-basic/`** — Same-file fixture
  - `cjpm.toml`, `src/main.cj`
- **`fixtures/cangjie/reference-function-call-cross-file/`** — Cross-file fixture
  - `cjpm.toml`, `src/main.cj`, `src/mathpkg/ops.cj`
- **`docs/plans/2026-05-06-cangjie-phase2-slice13-function-call-reference-preflight.md`**
- **`docs/plans/2026-05-06-cangjie-phase2-slice13-function-call-reference-execution-card.md`**

### 2.3 Forbidden write set — NOT touched

- GitNexus-RC runtime: no changes
- GitNexus-RC-Tool: no changes
- Live cangjie repo: no changes
- Cangjie index checkout: no changes
- MCP/HTTP/UI: no changes
- LSP: no changes
- Diagnostics runner: no changes
- Schema: no changes
- CLI contract: no changes
- Dependencies: zero new dependencies

## 3. Test results

### 3.1 New tests (function_call_reference.rs) — 10/10 pass

| Test | What it verifies |
|------|-----------------|
| `basic_fixture_parses_cleanly` | Fixture has no ERROR nodes |
| `basic_fixture_has_symbols` | Fixture has expected symbols |
| `same_file_function_call_produces_uses_edge` | `add(x, y)` → USES edge, confidence=0.80 |
| `constructor_call_produces_uses_edge` | `Point(0, 0)` → USES edge, includes Class kind |
| `builtin_constructor_no_edge` | Array/Int64 calls → no USES edge |
| `method_call_no_edge` | `calc.getValue()` → no USES edge (stop-line) |
| `unresolved_function_call_no_edge` | Undefined calls → no fake edges |
| `cross_file_function_call_via_import` | `add(1,2)` via import → cross-file USES, confidence=0.85 |
| `function_call_reference_targets_exist_in_graph` | Endpoint integrity (all ref edges) |
| `cross_file_function_call_endpoint_integrity` | Cross-file endpoint integrity |

### 3.2 All existing tests pass

- `cargo test`: 45 pass (no feature)
- `cargo test --features tree-sitter-cangjie`: 233 pass, 0 fail
- No regressions in reference_extraction, cross_file_reference, imports, manifests, modules, project_model

### 3.3 Formatting and lint

- `cargo fmt --check`: clean
- `cargo check`: only pre-existing dead_code warnings behind `#[cfg(feature)]` gate
  - `package_name_from_target` (imports.rs:301) — future helper
  - `BUILTIN_TYPES`, `is_builtin_type`, `TYPE_DECLARATION_KINDS`, `type_name_kind`, `FuncContext`, `SameFileIndex` (references.rs) — behind feature gate, used when feature is active

## 4. Architecture notes

### 4.1 AST handling

The function call handler is placed in the `postfixExpression` branch of `walk()`,
after the existing fieldAccess handler. The handler:

1. Checks `has_call_suffix(node)` — guards only postfixExpressions with callSuffix
2. Extracts callee name via `extract_callee_name()`
3. Filters builtin types (`is_builtin_type()`)
4. Pushes USES reference via `push_reference()` — same resolution pipeline as type annotations
5. Does NOT return early — recursive walk handles nested calls like `foo(bar())`

### 4.2 Callee name extraction

Three AST patterns handled:

- **Simple call** (`func(args)`): postfixExpression → [atomicVariable → varBindingPattern, callSuffix]
- **Qualified call** (`pkg.func(args)`): postfixExpression → [postfixExpression(...), callSuffix]; inner has fieldAccess
- **Method call** (`obj.method(args)`): detected and skipped (inner postfixExpression ends with fieldAccess, no callSuffix)

### 4.3 Confidence policy

| Scenario | Confidence | Rationale |
|----------|-----------|-----------|
| Same-file function call | 0.80 | Lower than type annotation resolve because function calls can be overloaded (no overload resolution yet) |
| Cross-file via explicit import | 0.85 | Explicit import has higher confidence for function calls than type annotations (function name is the import target itself) |

## 5. Risk assessment

- **No HIGH/CRITICAL risks introduced.**
- All new code is behind `#[cfg(feature = "tree-sitter-cangjie")]`.
- Method call detection is conservative (returns None for ambiguous cases).
- Endpoint integrity: all reference targets exist in the graph node set.
- No fake edges on ambiguity: same-file resolve fails → import binding resolve fails → no edge emitted.

## 6. What's next

Slice 13 is the **last currently scoped feature slice** for Cangjie Phase 2.
Future work categories (all require new preflight):

1. **Slice 14: Wildcard import deep expansion** — `import mathpkg.ops.*` → resolve all public symbols
2. **Slice 15: Alias-resolution** — `import mathpkg.ops.{add as plus}` → rename tracking
3. **Slice 16: Method dispatch** — `obj.method()` → requires type inference
4. **Slice 17: Diagnostics runner integration** — hook references into diagnostics
5. **Slice 18: LSP integration** — reference-based go-to-definition

Each above requires its own preflight/gate — none should be implemented directly.

## 7. Approval

- Preflight: PROCEED recommended, scope bounded
- Execution card: 10-step plan completed
- Implementation: delivered, all tests pass
- This closure review: complete
