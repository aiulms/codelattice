# Preflight: Cangjie Constructor/Interface/Call-Reference Bounded Quality Enhancement

> **日期：** 2026-05-10
> **类型：** Preflight
> **状态：** ✅ Pass — Bounded improvement confirmed safe
> **Base commit：** `1b74e3f`

---

## 1. Goal

Harden Cangjie constructor, interface, and call-reference quality within existing stop-lines. No runtime changes expected — test hardening only.

## 2. Inventory Findings

### 2.1 Graph Structure

Cangjie graph uses 8 `EdgeKind` variants:
- `ContainsPackage`, `OwnsSource` — structural hierarchy
- `Defines` — SourceFile → Symbol (including class/interface members)
- `Annotates`, `Uses`, `Accesses`, `Modifies`, `Imports` — reference edges

**No `MemberOf` edge** — class/interface members are defined via `Defines` from SourceFile to Symbol, not via a membership edge.

### 2.2 Constructor Symbol Extraction

- Init symbols extracted correctly from class/struct definitions
- Labels follow pattern: `<ClassName>.init` (e.g., `AppConfig.init`, `Point.init`)
- Constructor CALLS (e.g., `AppConfig("test", 42)`) resolve to **Class** symbol, not Init symbol
- This is by design: without full type inference, constructor calls cannot distinguish between constructors — the call resolves to the class itself
- Tests in `crates/cangjie/tests/constructor_extraction.rs` verify Init node presence and endpoint integrity

### 2.3 Test Coverage

Existing Cangjie test suite:
- `constructor_extraction.rs` — Init symbol extraction, endpoint integrity
- `function_call_reference.rs` — USES edges for function/constructor calls
- `reference_extraction.rs` — USES/ACCESSES/MODIFIES edge extraction
- `cross_file_reference.rs` — cross-file reference resolution
- `import_resolution.rs` — import binding
- `graph_contract.rs` — deterministic graph output
- `endpoint_integrity.rs` — 0 dangling edges
- `graph_parity_smoke.rs` — production fixture smoke

### 2.4 Fixture Coverage

10 Cangjie fixtures:
- `constructor-basic`, `constructor-cross-file` — constructor extraction
- `reference-function-call-basic`, `reference-function-call-cross-file` — function call refs
- `references-basic`, `reference-cross-file-basic` — general reference extraction
- `imports-basic` — import resolution
- `cjpm-basic`, `cjpm-workspace` — project model
- `portable-smoke` — portability verification

### 2.5 Constructor Call Resolution Design

Current behavior:
```
AppConfig("test", 42) → Uses edge → AppConfig (Class symbol)
```

This is correct within stop-lines:
- No full type inference → cannot distinguish overloaded constructors
- Call resolves to class symbol → Class is the only unambiguous target
- Init symbols exist as separate nodes for reference but are not call targets

## 3. Bounded Changes

| # | Change | Type | Risk |
|---|--------|------|------|
| 1 | Add constructor call target assertion (verify Uses edge targets Class) | Test | None |
| 2 | Add interface method call assertion (verify Uses edge for interface methods) | Test | None |
| 3 | Add confidence/reason assertions for cross-file constructor calls | Test | None |

## 4. Stop-line Check

- ❌ No full type inference
- ❌ No full interface/trait solving
- ❌ No macro expansion
- ❌ No LSP daemon integration
- ❌ No runtime code changes
- ✅ Test hardening only

## 5. Verdict

**PASS.** Cangjie constructor/interface quality is already at ceiling within stop-lines. Changes are:
1. Add targeted test assertions for constructor call → Class symbol targeting
2. Verify interface method call edge quality
3. Close with documentation update

No runtime changes required. Constructor calls resolving to Class symbol is correct by design.
