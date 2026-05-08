# Stage B — Function Source ID Mapping Closure Review

**Date:** 2026-05-08  
**Status:** Closure Review  
**Type:** Production Graph Quality Hardening  
**Parent:** Stage B — Function synthetic node reduction

---

## Root Cause

`emit_cangjie_reference_edges()` 构建了 `constructor_to_symbol_id`（Constructor→Init）和 `method_to_symbol_id`（Method→Function）映射，但缺少 `function_to_symbol_id` 映射。所有顶级函数调用产生的 `Function:<abs-path>:<funcName>#<arity>` source ID 永远无法解析为真实 Symbol node，始终保留为 synthetic node。

## Fix

在 `graph.rs` 中新增 `function_to_symbol_id: HashMap<String, String>` 映射：

```
Function:<abs-path>:<funcName>#<arity> → sym:<rel-path>:Function:<funcName>#<arity>
```

`resolve_source_id()` 新增 `Function:` 前缀处理（精确匹配 + arity fallback），与 Constructor:/Method: 处理方式一致。

## Changes

### `crates/cangjie/src/graph.rs`

1. `emit_cangjie_reference_edges()`: 新增 `function_to_symbol_id` HashMap，在 symbol loop 中为 `owner_name.is_none()` 的 Function symbol 填充映射
2. `resolve_source_id()`: 新增 `function_to_symbol_id` 参数，添加 `Function:` 前缀处理
3. 新增 3 个单元测试

## Before/After

| Target | Function Synthetic Before | Function Synthetic After |
|--------|--------------------------|--------------------------|
| cjgui (GitNexus-Index) | 521 | **0** |
| cjgui (cangjie) | 985 | **0** |
| web_framework | 2 | **0** |
| json_parser | 0 | **0** |
| **Total** | **1508** | **0** |

## Integrity Verification

- Duplicate node IDs: 0 (unchanged)
- Duplicate edge triples: 0 (unchanged)
- Dangling source edges: 0 (unchanged)
- Dangling target edges: 0 (unchanged)
- Output deterministic: true (unchanged)
- Constructor synthetic: 0 (unchanged)
- Method synthetic: 0 (unchanged)
- Total nodes: 3411 (down from 4919, no more CallableSource nodes)

## Tests

- `cargo test --features tree-sitter-cangjie`: 112 lib tests + all suites pass (0 fail)
- 3 new unit tests: `function_source_id_maps_to_symbol_node_id`, `function_source_id_maps_without_arity_fallback`, `constructor_and_method_mapping_still_works`
- endpoint_integrity: 12/12 pass
- constructor_extraction: 12/12 pass
- multi_project_smoke: 4/4 targets pass

## Stop-lines compliance

- ✅ No change to GitNexus-RC
- ✅ No change to GitNexus-RC-Tool
- ✅ No change to live repo
- ✅ No destructive git operations
- ✅ No new dependencies
- ✅ Synthetic fallback preserved (unresolved source IDs still get synthetic nodes)
- ✅ No symbol extraction changes
- ✅ No reference extraction changes
- ✅ No breaking node ID format changes

## What remains synthetic (correctly)

- External function calls (stdlib/core) — not in project symbol index, no edge emitted
- Lambda/anonymous functions — no symbol definition, correctly unmapped
- Builtin function calls — filtered in reference extraction, no Uses edge

**Stage B Priority 2 状态：** ✅ 完成
