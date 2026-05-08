# Stage B — Function Synthetic Node Root Cause Audit & Mapping Preflight

**Date:** 2026-05-08  
**Status:** Preflight  
**Type:** Production Graph Quality Hardening  
**Parent:** Phase 2 autonomous advancement — Stage B Function synthetic node reduction

---

## Production Smoke Baseline

| Target | Function Synthetic | Init Symbols | Total Symbols |
|--------|-------------------|--------------|---------------|
| cjgui (GitNexus-Index) | 521 | 188 | 887 |
| cjgui (cangjie) | 985 | 567 | 2109 |
| web_framework | 2 | 13 | 143 |
| json_parser | 0 | 8 | 144 |

json_parser has 0 Function synthetic because the previous owner+arity hardening resolved all methods there.

## Root Cause Analysis

### Source ID generation (`references.rs` `build_source_id()`)

| Enclosing context | Source ID format |
|---|---|
| Constructor (init with owner) | `Constructor:<abs-path>:<Owner>.init#<arity>` |
| Method (function with owner) | `Method:<abs-path>:<Owner>.<funcName>#<arity>` |
| Top-level function (no owner) | `Function:<abs-path>:<funcName>#<arity>` |

### Resolution mapping (`graph.rs` `resolve_source_id()`)

| Source ID prefix | Mapping exists? | Status |
|---|---|---|
| `Constructor:` | `constructor_to_symbol_id` | ✅ Resolved |
| `Method:` | `method_to_symbol_id` | ✅ Resolved |
| `Function:` | **NONE** | ❌ Always synthetic |

### Gap

`emit_cangjie_reference_edges()` builds `constructor_to_symbol_id` for Init symbols and `method_to_symbol_id` for Function symbols with owner_name, but **has no `function_to_symbol_id` mapping for top-level Function symbols** (owner_name = None).

All top-level function calls produce `Function:` source IDs that are never resolved to real Symbol nodes, regardless of whether the symbol exists in the project.

### Pattern classification

All 500+ Function synthetic nodes in cjgui are expected to be:
1. **Top-level function calls** — source ID is `Function:<abs-path>:<funcName>#<arity>`, matching Function symbol exists in same project, but no mapping resolves them. **Fixable.**
2. **External function calls** — functions from std/core/external packages. Symbol extraction doesn't cover these. **Correctly synthetic.**
3. **Lambda/anonymous functions** — no symbol definition. **Correctly synthetic.**

## Fix Strategy

### Add `function_to_symbol_id` mapping

In `emit_cangjie_reference_edges()`, add a `function_to_symbol_id: HashMap<String, String>` for Function symbols with `owner_name.is_none()`:

```
Function:<abs-path>:<funcName>#<arity> → sym:<rel-path>:Function:<funcName>#<arity>
```

### Update `resolve_source_id()`

Add `Function:` prefix handling:
1. Exact match on `function_to_symbol_id`
2. Fallback: strip `#arity` suffix and retry

### What this does NOT change

- Does NOT change symbol extraction
- Does NOT change reference extraction (build_source_id)
- Does NOT remove synthetic fallback — unmapped Function IDs still get synthetic nodes
- Does NOT touch Constructor or Method paths
- Arity-based mapping consistent with existing Init/Method patterns

## Implementation scope

### `crates/cangjie/src/graph.rs`

1. Add `function_to_symbol_id: HashMap<String, String>` to `emit_cangjie_reference_edges()`
2. Populate it in the symbol loop for Function symbols without owner_name
3. Update `resolve_source_id()` signature to accept `function_to_symbol_id`
4. Add `Function:` prefix resolution logic in `resolve_source_id()`

### Tests

1. Unit test: Function source_id → symbol node_id mapping
2. Integration test: endpoint_integrity regression on constructor-basic fixture
3. Production smoke: verify Function synthetic reduction, no regression

## Stop-lines

- ❌ No change to GitNexus-RC
- ❌ No change to GitNexus-RC-Tool
- ❌ No change to live repo
- ❌ No destructive git operations
- ❌ No new dependencies
- ❌ No removal of synthetic fallback
- ❌ No symbol extraction changes
- ❌ No reference extraction changes
- ❌ No breaking changes to node ID format

## Expected impact

- Function synthetic nodes should decrease materially in cjgui targets (most toplevel function calls now resolve)
- Lambda/anonymous/external Function synthetic nodes will remain (correct)
- Constructor=0, Method=0 should remain unchanged
- Duplicate nodes, dangling edges must remain 0
- Output must remain deterministic

## Verification

- `cargo fmt --check`
- `git diff --check`
- `cargo test --features tree-sitter-cangjie`
- `cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`
- `cargo test --features tree-sitter-cangjie --test endpoint_integrity -- --nocapture`
- `cargo test --features tree-sitter-cangjie --test constructor_extraction -- --nocapture`
