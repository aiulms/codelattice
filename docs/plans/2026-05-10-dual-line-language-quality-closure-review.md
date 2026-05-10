# Closure Review: Rust + Cangjie Dual-Line Language Quality Enhancement

> **日期：** 2026-05-10
> **类型：** Closure Review
> **Base commit：** `1b74e3f`
> **Status：** ✅ PASS — Bounded quality enhancement completed

---

## 1. Summary

Two parallel bounded improvements completed:
1. **Rust method/associated call quality hardening** — stale fixture comment fixed, 4 targeted regression tests added
2. **Cangjie constructor/interface quality hardening** — 2 targeted regression tests added

**No runtime code changes.** All changes are documentation accuracy and test coverage.

## 2. Changes

### 2.1 Rust Line

| File | Change | Type |
|------|--------|------|
| `fixtures/call-resolution/c11-receiver-type/src/lib.rs` | Fixed stale comment: "not supported" → "resolved via receiver type" | Doc |
| `crates/cli/tests/project_model_call_expected_compare.rs` | Added 4 targeted regression tests | Test |
| `docs/plans/2026-05-10-rust-method-associated-call-preflight.md` | Preflight documentation | Doc |

**New tests added:**
- `receiver_type_method_confidence_contract` — validates c11 Vec/str/param receiver type resolution at 0.65
- `constructor_chain_inference_confidence_contract` — validates c12 Vec/HashMap/String constructor chain inference
- `associated_fn_confidence_contract` — validates c6 Config::new and name.to_string confidence/reason
- `associated_fn_disambiguation_contract` — validates c15 DataProcessor::build and RequestHandler::build disambiguation

### 2.2 Cangjie Line

| File | Change | Type |
|------|--------|------|
| `crates/cangjie/tests/constructor_extraction.rs` | Added 2 targeted regression tests | Test |
| `docs/plans/2026-05-10-cangjie-constructor-interface-call-preflight.md` | Preflight documentation | Doc |

**New tests added:**
- `test_constructor_call_targets_class_symbol_not_init` — validates constructor call Uses edges target Class symbols (not Init)
- `test_init_symbols_defined_from_source_file` — validates Init symbols are Defined from SourceFile nodes

## 3. Key Findings

### 3.1 Rust Method Resolution — Already Mature

The 5-stage resolution pipeline is complete and working correctly:
1. Exact match (0.85-0.90) → 2. Associated fn (0.70-0.75) → 3. Method name blind (0.65) → 4. Stdlib trait (0.55) → 5. Receiver type (0.65)

**Notable discovery:** c11 fixture had a stale comment claiming function parameter type annotations are "not supported in Phase 2", but `name.len()` IS resolved at 0.65 via `call-receiver-type-method-resolved`. `scan_variable_type_annotation` handles function parameters correctly.

### 3.2 Cangjie Constructor Calls — Correct by Design

Constructor calls (e.g., `AppConfig("test", 42)`) resolve to **Class** symbol, not Init symbol. This is correct:
- Without full type inference, constructor calls cannot distinguish overloaded inits
- Class is the only unambiguous target
- Init symbols exist as separate nodes for structural definition, not call resolution

### 3.3 Cangjie Graph Structure

8 edge types, no MemberOf edge. Class/interface members defined via `Defines` from SourceFile to Symbol.

## 4. Stop-line Verification

All changes are within stop-lines:

| Stop-line | Status |
|-----------|--------|
| No rust-analyzer | ✅ |
| No type inference | ✅ |
| No trait solving | ✅ |
| No macro expansion | ✅ |
| No cargo metadata | ✅ |
| No full type inference (Cangjie) | ✅ |
| No runtime code changes | ✅ |
| No new dependencies | ✅ |

## 5. Test Coverage Summary

### Rust (pre-existing)
- 24 call-resolution fixtures with `expected-calls.json`
- `call_comparison_passes_for_all_fixtures` validates all fields
- 19 call forms covered by confidence matrix

### Rust (new)
- 4 targeted confidence/reason regression tests for key call forms:
  - Receiver type method (c11) — including function parameter case
  - Constructor chain inference (c12) — Vec/String/HashMap
  - Associated function (c6) — local + stdlib trait
  - Associated function disambiguation (c15) — impl target filtering

### Cangjie (pre-existing)
- constructor_extraction.rs — 10 tests (Init extraction, endpoint integrity, synthetic reduction)
- function_call_reference.rs — USES edge extraction
- reference_extraction.rs — USES/ACCESSES/MODIFIES
- endpoint_integrity.rs — 0 dangling

### Cangjie (new)
- 2 targeted regression tests for constructor call design contracts:
  - Constructor call → Class symbol targeting
  - Init symbol → SourceFile Defines edge source

## 6. Assessment

**Quality ceiling reached within stop-lines.** Both Rust and Cangjie call resolution are at their maximum achievable quality without crossing into type inference or trait solving territory. The changes in this round are documentation accuracy and test coverage hardening only.

## 7. Next Steps

- Continue alpha trial with current quality baseline
- Beta readiness requires: type inference integration (post-MVP, outside stop-lines)
- Consider external AI Run #003 results for additional quality signals
