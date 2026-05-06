# Cangjie Phase 2 Slice 13 — Function Call Reference Extraction Execution Card

**Date:** 2026-05-06
**Type:** execution card
**Status:** 进行中
**Preflight:** `2026-05-06-cangjie-phase2-slice13-function-call-reference-preflight.md`

## 1. Scope Freeze

Extend Cangjie reference extraction from type-annotation-only to include function call references. When the AST walker encounters `postfixExpression` with `callSuffix`, extract the callee name and resolve via same-file index → cross-file ImportBindingTable.

**MVP forms:**
- Simple function call: `func(args)` → callee = func
- Constructor call: `Type(args)` → callee = Type
- Qualified call: `pkg.func(args)` → callee = func (last segment)
- Same-file + cross-file via explicit import

**Excluded forms** (by design):
- Method call (`obj.method(args)`) — requires type inference
- Wildcard import call — wildcard expansion not supported
- Alias renamed import call — alias rename not supported
- External version/git dep call — external dep resolution not supported

## 2. Implementation Steps

### Step 1: `extract_callee_name()` helper (~30 lines)

New function in references.rs:

```rust
/// Extract the callee name from a postfixExpression that has a callSuffix.
/// Returns None for method calls (obj.method()) which require type inference.
fn extract_callee_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let children = named_children(node);
    if children.is_empty() {
        return None;
    }

    // Case 1: Simple call — func(args) or Type(args)
    //   postfixExpression → [atomicVariable, callSuffix]
    if children[0].kind() == "atomicVariable" {
        let var = find_named_child_by_kind(children[0], "varBindingPattern");
        return var.and_then(|v| v.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()));
    }

    // Case 2: Qualified call — pkg.func(args)
    //   postfixExpression → [postfixExpression(atomicVariable + fieldAccess), callSuffix]
    //   Extract the LAST fieldAccess segment as callee
    if children[0].kind() == "postfixExpression" {
        let inner_children = named_children(children[0]);
        // Method call detection: if inner postfixExpression ends with fieldAccess
        // and does NOT itself have callSuffix → it's a method call, skip
        if let Some(last) = inner_children.last() {
            if last.kind() == "fieldAccess"
                && (inner_children.len() < 2
                    || inner_children[inner_children.len() - 2].kind() != "callSuffix")
            {
                // This is obj.method() — method call, NOT supported
                return None;
            }
        }
        // Qualified call: extract from fieldAccess
        let field = find_last_named_child_by_kind(children[0], "fieldAccess");
        if let Some(fa) = field {
            let av = find_named_child_by_kind(fa, "atomicVariable");
            if let Some(av_node) = av {
                let vb = find_named_child_by_kind(av_node, "varBindingPattern");
                return vb
                    .and_then(|v| v.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()));
            }
        }
    }

    None
}
```

### Step 2: `has_call_suffix()` helper (~8 lines)

```rust
/// Check if a postfixExpression node has a callSuffix (i.e., is a function call).
fn has_call_suffix(node: tree_sitter::Node) -> bool {
    let children = named_children(node);
    children.last().map_or(false, |c| c.kind() == "callSuffix")
}
```

### Step 3: Function call handler in `walk()` (~50 lines)

Insert AFTER the existing postfixExpression/fieldAccess handler (line 761) and BEFORE the type annotation handlers:

```rust
// ── Function call: postfixExpression with callSuffix ──
if kind == "postfixExpression" && has_call_suffix(node) {
    if let Some(callee_name) = extract_callee_name(node, source) {
        // Skip builtin types (e.g., Array(10))
        if !is_builtin_type(&callee_name) {
            let source_id = build_source_id(func_stack.last(), file_path);
            push_reference(
                references,
                ReferenceKind::Uses,
                source_id,
                &callee_name,
                vec![
                    CangjieSymbolKind::Function,
                    CangjieSymbolKind::Class,   // constructor call
                    CangjieSymbolKind::Struct,  // struct literal call
                ],
                file_path,
                index,
                import_bindings,
                0.80, // same-file function call: confidence 0.80
                "cangjie-function-call",
            );
        }
    }
    // Don't return here — continue walking children
    // (the callee expression may contain nested calls like foo(bar()))
}
```

### Step 4: Update `walk()` for nested calls

The `postfixExpression` + `callSuffix` handler should NOT return early. Instead, fall through to the recursive walk at the end of the function to handle nested call expressions (e.g., `foo(bar(baz))`).

Add a guard: if the node was already processed as a function call, skip re-processing its children as field read (they're already covered).

### Step 5: Fixture `reference-function-call-basic/` (3 files)

**`fixtures/cangjie/reference-function-call-basic/cjpm.toml`:**
```toml
[package]
name = "reference-function-call-basic"
version = "0.1.0"
src-dir = "src"

[dependencies]
```

**`fixtures/cangjie/reference-function-call-basic/src/main.cj`:**
```cj
// Same-file function call reference fixture

func add(a: Int64, b: Int64): Int64 {
    a + b
}

func compute(x: Int64, y: Int64): Int64 {
    // Function call: add(x, y) → USES edge to add
    add(x, y)
}

class Point {
    var x: Int64
    var y: Int64
    public init(x: Int64, y: Int64) {
        this.x = x
        this.y = y
    }
}

func createOrigin(): Point {
    // Constructor call: Point(0, 0) → USES edge to Point
    Point(0, 0)
}

main(): Int64 {
    let result = compute(1, 2)
    0
}
```

### Step 6: Fixture `reference-function-call-cross-file/` (3 files)

**`fixtures/cangjie/reference-function-call-cross-file/cjpm.toml`:**
```toml
[package]
name = "reference-function-call-cross-file"
version = "0.1.0"
src-dir = "src"

[dependencies]
```

**`fixtures/cangjie/reference-function-call-cross-file/src/mathpkg/ops.cj`:**
```cj
package mathpkg

public func add(a: Int64, b: Int64): Int64 {
    a + b
}

public func multiply(a: Int64, b: Int64): Int64 {
    a * b
}
```

**`fixtures/cangjie/reference-function-call-cross-file/src/main.cj`:**
```cj
import mathpkg.ops.{add}

main(): Int64 {
    // Cross-file function call: add(1, 2) → USES edge to mathpkg/ops.cj add
    add(1, 2)
}
```

### Step 7: Integration tests `tests/function_call_reference.rs` (~150 lines)

Feature-gated test module with the following tests:

1. **`same_file_function_call_produces_uses_edge`**: Verify `add(x, y)` → USES edge with target_name="add", reason="cangjie-function-call", confidence=0.80
2. **`constructor_call_produces_uses_edge`**: Verify `Point(0, 0)` → USES edge with target_name="Point", target_kinds includes Class
3. **`builtin_constructor_no_edge`**: Array/Int64 constructor call → no USES edge
4. **`cross_file_function_call_via_import`**: Verify import mathpkg.ops.{add} → add(1,2) → cross-file USES edge with confidence=0.75, target_file contains "ops.cj"
5. **`unresolved_function_call_no_edge`**: Calling undefined function → no USES edge
6. **`method_call_no_edge`**: `obj.method()` → no USES edge（method dispatch stop-line）
7. **`endpoint_integrity`**: All USES edges from function calls have targets in graph node set

### Step 8: Update `inspect_cangjie_project()` pipeline

No changes needed. `inspect_cangjie_project()` already builds ImportBindingTable before calling `extract_cangjie_references()`. Function call references automatically benefit from cross-file resolution.

### Step 9: Run verification

```bash
cargo fmt --check
cargo check
cargo test                          # 95+ tests pass without feature
cargo test --features tree-sitter-cangjie  # 108+ tests pass with feature
```

### Step 10: Docs update

- `docs/plans/README.md` — add Slice 13 entry
- GitNexus-RC `docs/language-support/TASK_TRACKER.md` — milestone sync
- GitNexus-RC `docs/language-support/plans/README.md` — milestone sync

## 3. Write Set Summary

| File | Action | Lines |
|------|--------|-------|
| `crates/cangjie/src/extractors/references.rs` | Modify | +80 |
| `fixtures/cangjie/reference-function-call-basic/` | Create | 3 files, ~40 lines |
| `fixtures/cangjie/reference-function-call-cross-file/` | Create | 3 files, ~25 lines |
| `crates/cangjie/tests/function_call_reference.rs` | Create | ~150 lines |
| `docs/plans/2026-05-06-cangjie-phase2-slice13-*.md` | Create | preflight + execution-card + closure-review |
| `docs/plans/README.md` | Modify | +15 lines |
| GitNexus-RC `TASK_TRACKER.md` + `plans/README.md` | Modify | +20 lines |

**Total: ~330 lines new + ~35 lines docs sync**

## 4. Forbidden Set

- [ ] GitNexus-RC runtime — NOT TOUCHED
- [ ] Tool checkout — NOT TOUCHED
- [ ] Cangjie live repo — NOT TOUCHED
- [ ] MCP/HTTP/UI — NOT TOUCHED
- [ ] Cangjie LSP client — NOT TOUCHED
- [ ] diagnostics runner — NOT TOUCHED
- [ ] graph.rs EdgeKind enum — NO NEW VARIANTS
- [ ] project model schema — NOT TOUCHED
- [ ] imports.rs — NOT TOUCHED
- [ ] subprocess/cjpm_tree.rs — NOT TOUCHED
- [ ] Cargo.toml dependencies — NO NEW DEPS
- [ ] Feature gate boundary — NO LEAKAGE

## 5. Acceptance Criteria

- [ ] `cargo fmt --check` clean
- [ ] `cargo test` 95+ pass（without feature）
- [ ] `cargo test --features tree-sitter-cangjie` 108+ pass（all existing + 7 new）
- [ ] Same-file function call → USES edge, reason="cangjie-function-call", confidence=0.80
- [ ] Constructor call → USES edge
- [ ] Cross-file function call via import → USES edge, confidence=0.75
- [ ] Unresolved function call → no edge
- [ ] Method call → no edge
- [ ] Builtin type constructor → no edge
- [ ] Endpoint integrity: all USES edges have valid source/target in graph
- [ ] Zero new dependencies
- [ ] Feature gate maintained

## 6. Stop-lines

- No method dispatch（type inference stop-line）
- No new EdgeKind variant
- No parameter/overload matching
- No external dependency resolution
- No GitNexus-RC runtime/Tool/live repo modification
