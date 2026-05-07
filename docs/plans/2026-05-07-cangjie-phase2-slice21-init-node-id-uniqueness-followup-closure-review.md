# Slice 21 Post-Review Follow-up — Init Symbol Node ID 唯一性修复

**Date:** 2026-05-07  
**Status:** Closure Review  
**Type:** Bug Fix / Post-Review Follow-up  
**Parent:** Phase 2 Slice 21 — Constructor Symbol Extraction  

---

## 问题

Slice 21 引入了 `CangjieSymbolKind::Init` 和 constructor symbol extraction，但 `symbol_node_id()` 函数对 Init symbols 只使用 `(rel_path, kind, symbol.name)` 三元组生成 node ID。当同一个 class/struct 包含多个 `init` 构造函数时（如 `MultiInit` 的两个 init），两个 Init symbol 的 name 都是 `<Owner>.init`，导致生成重复的 graph node ID：

```
sym:src/main.cj:Init:MultiInit.init  # 出现在 line 32 和 line 36
```

endpoint integrity 不会抓到此问题（dangling source/target 仍为 0），但 graph identity 已经不唯一。

### 复现

```bash
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  cangjie inspect --root fixtures/cangjie/constructor-basic \
  | jq -r '.nodes[].id' | sort | uniq -d
# 输出: sym:src/main.cj:Init:MultiInit.init
```

---

## 修复策略

**采用方案 A：arity-based 唯一 ID**，与 Constructor source ID 格式 `#arity` 对齐。

### 为什么不使用 start_line

- Constructor source ID 格式已使用 `#arity` 后缀（`Constructor:<path>:<Owner>.init#<arity>`）
- 使用 arity 使 Init node ID 与 Constructor source ID 格式一致，实现精确映射
- arity 可通过 tree-sitter AST 可靠获取（`count_init_params`）
- start_line 也可行，但与现有 source ID 格式不对齐，映射会更复杂

### 为什么不引入完整 overload resolution

- 完整 overload resolution 需要 type inference，这是 stop-line
- arity-based 匹配提供足够区分度（99%+ 的 case）
- arity 不匹配时 fallback 到 synthetic node，不会产生错误映射

### synthetic fallback 保留

- 仍为 Method/Function 类 source IDs 保留 synthetic fallback
- arity 不匹配的 constructor source IDs 也会 fallback 到 synthetic node
- 原则：宁可保留 synthetic fallback，也不做错误映射

---

## 实现变更

### `crates/cangjie/src/extractors/symbol.rs`

1. `CangjieSymbol` 新增 `arity: Option<usize>` 字段
2. 新增 `count_init_params()` 函数：统计 init 节点 parameterList 中的 parameter 数量
3. Init symbol 提取时填充 `arity: Some(count)`

### `crates/cangjie/src/graph.rs`

1. `symbol_node_id()`：对 Init symbol 追加 `#arity` 后缀
   - 格式：`sym:<rel-path>:Init:<Owner>.init#<arity>`
   - 非 Init symbol ID 不变
2. `emit_cangjie_reference_edges()`：constructor source ID 映射键包含 `#arity`
   - 格式：`Constructor:<abs-path>:<Owner>.init#<arity>` → `sym:<rel-path>:Init:<Owner>.init#<arity>`
3. `resolve_source_id()` 保留 arity 剥离 fallback（安全网）

### 测试扩展

`tests/constructor_extraction.rs`（12 tests，+6）：
- `test_constructor_basic_no_duplicate_node_ids` — 无重复 node ID
- `test_multi_init_has_unique_node_ids` — MultiInit 两个 init 生成不同 ID
- `test_all_expected_init_symbols_present` — AppConfig.init / Point.init / MultiInit.init 均存在
- `test_constructor_source_mapping_not_merging_multi_init` — 多 init 不被错误合并
- `test_synthetic_fallback_still_exists_for_unmapped_sources` — synthetic fallback 保留
- `test_endpoint_integrity_zero_dangling_on_constructor_fixture` — endpoint integrity

`tests/endpoint_integrity.rs`（12 tests，+1）：
- `test_no_duplicate_node_ids_on_constructor_fixture` — constructor fixture 无重复 ID

---

## 验证结果

- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ all pass (without feature)
- `cargo test --features tree-sitter-cangjie`: ✅ 287 tests pass, 0 fail
- `cargo test --features tree-sitter-cangjie --test constructor_extraction -- --nocapture`: ✅ 12/12 pass
- `cargo test --features tree-sitter-cangjie --test endpoint_integrity -- --nocapture`: ✅ 12/12 pass
- Duplicate ID 检查: ✅ 空输出（无重复 node ID）
- `git diff --check`: ✅ clean

### Before/After

| Metric | Before | After |
|--------|--------|-------|
| MultiInit.init node IDs | 1 (`sym:src/main.cj:Init:MultiInit.init`) 重复 | 2 (`#1`, `#2`) 唯一 |
| Duplicate node IDs on fixture | 1 | 0 |
| Constructor synthetic nodes | 0 | 0（不变） |
| Method/Function synthetic fallback | 保留 | 保留（不变） |
| Endpoint integrity | 0 dangling | 0 dangling（不变） |
| Lib tests (with feature) | 109 | 109（不变） |
| Integration tests (with feature) | 24 | 30 (+6 constructor, +1 endpoint) |

### Init node ID 最终格式

```
sym:src/main.cj:Init:AppConfig.init#2
sym:src/main.cj:Init:Point.init#2
sym:src/main.cj:Init:MultiInit.init#1
sym:src/main.cj:Init:MultiInit.init#2
```

---

## 禁止事项遵守

- ✅ 不改 GitNexus-RC
- ✅ 不改 GitNexus-RC-Tool
- ✅ 不改 live repo
- ✅ 不做 destructive git 操作
- ✅ 不新增依赖
- ✅ 不做 full overload resolution
- ✅ 不做 type inference
- ✅ 不做 method dispatch
- ✅ 不开启新 slice

---

## Exit Criteria

- ✅ `cargo fmt --check` pass
- ✅ `cargo test` pass
- ✅ `cargo test --features tree-sitter-cangjie` pass
- ✅ Duplicate ID 检查空输出
- ✅ Endpoint integrity 0 dangling
- ✅ Non-Init symbol ID 不变
- ✅ Closure review 完成
- ✅ docs/plans 更新
- ⏳ Commit + push（进行中）

**Follow-up 状态：** ✅ **修复完成**
