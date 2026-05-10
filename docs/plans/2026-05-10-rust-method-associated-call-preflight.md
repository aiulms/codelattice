# Preflight: Rust Method/Associated Call Bounded Quality Enhancement

> **日期：** 2026-05-10
> **类型：** Preflight
> **状态：** ✅ Pass — Bounded improvement confirmed safe
> **Base commit：** `1b74e3f`

---

## 1. Goal

Harden Rust method/associated call quality within existing stop-lines. No runtime changes expected — documentation and test hardening only.

## 2. Inventory Findings

### 2.1 Current Resolution Quality (already mature)

Rust call resolution implements a 5-stage pipeline:

1. **Exact match** (0.85-0.90): same-module, import, crate::/self::/super:: paths
2. **Associated fn** (0.70-0.75): unique/multi disambiguation
3. **Method name blind** (0.65): local crate method match
4. **Stdlib trait method** (0.55): STDLIB_TYPE_METHODS table lookup
5. **Receiver type method** (0.65): `scan_variable_type_annotation` → `lookup_receiver_type_method`
6. **Constructor chain** (0.65): `let x = T::new()` → `x.method()` resolved via constructor return type

### 2.2 Fixture Coverage

24 fixtures with `expected-calls.json` covering all 19 call forms:
- **c11-receiver-type**: 14 calls — Vec/str/Option/Result/String type-annotated methods at 0.65
- **c12-let-constructor-method**: 14 calls — constructor chain inference for Vec/String/HashMap at 0.65
- **c6-associated-fn**: Config::new at 0.75, name.to_string at 0.55 (stdlib trait)
- **c7-method-call**: c.increment at 0.65 (local crate method)
- **c15-associated-function-disambiguation**: DataProcessor::build and RequestHandler::build at 0.75

### 2.3 Key Finding: c11 Stale Comment

File: `fixtures/call-resolution/c11-receiver-type/src/lib.rs` lines 46-48

```
// Function parameter — not supported in Phase 2
pub fn param_method(name: &str) {
    name.len(); // unresolved: name is a parameter, not let binding
}
```

**Reality**: `name.len()` IS resolved at 0.65 with reason `call-receiver-type-method-resolved` in expected-calls.json. The comment is stale — `scan_variable_type_annotation` handles function parameters correctly.

### 2.4 Confidence Matrix

All 19 call forms documented in `docs/architecture/rust-calls-confidence-matrix.md` v1.0.0. No gaps found.

## 3. Bounded Changes

| # | Change | Type | Risk |
|---|--------|------|------|
| 1 | Fix stale c11 comment: "not supported" → "resolved via receiver type" | Doc | None |
| 2 | Add c11 param_method assertion in expected-calls.json (verify it's already there) | Verify | None |
| 3 | Add confidence/reason regression test assertions for c12 constructor chain | Test | None |

## 4. Stop-line Check

- ❌ No rust-analyzer
- ❌ No type inference
- ❌ No trait solving
- ❌ No macro expansion
- ❌ No cargo metadata
- ❌ No runtime code changes
- ✅ Documentation and test hardening only

## 5. Verdict

**PASS.** Rust method resolution is already mature. Changes are:
1. Fix stale fixture comment (documentation accuracy)
2. Verify existing test coverage is complete (no action if already passing)
3. Close with documentation update

No runtime changes required.
