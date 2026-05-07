# Slice 21 Execution Card — Cangjie Constructor Symbol Extraction

**Date:** 2026-05-07  
**Status:** Execution Card  
**Type:** Feature / Semantic Upgrade  
**Slice ID:** Phase 2 Slice 21  

## Decision Summary

**方案 C3：Init Symbol Extraction + Source ID 映射 + Synthetic Fallback**

核心策略：
1. 新增 `CangjieSymbolKind::Init`，提取 class/struct body 中的 `init` 定义
2. CangjieSymbol 新增 `owner_name: Option<String>` 字段，支持 `Owner.init` 命名
3. Init symbol 的 node ID 格式：`sym:<rel-path>:Init:<Owner>.init`（保持 sym: 前缀）
4. 在 graph emission 中建立 Constructor source_id → init symbol node_id 映射
5. emit_cangjie_reference_edges 中，将 `Constructor:<abs-path>:<Owner>.init#arity` 映射为 `sym:<rel-path>:Init:<Owner>.init`
6. emit_synthetic_source_nodes 只为未被真实 init symbol 覆盖的 source IDs 发 synthetic node
7. Endpoint integrity 仍为 0 dangling

## Grammar Validation

- tree-sitter-cangjie 中 init 节点类型为 `"init"`（已确认 parser.c 中存在）
- init 节点是 classDefinition/structDefinition 的 named child
- 需要新增 tree-sitter query 模式来捕获 init 节点

## Write Set

| 文件 | 变更内容 |
|------|----------|
| `crates/cangjie/src/extractors/symbol.rs` | 1. 新增 `CangjieSymbolKind::Init`<br>2. CangjieSymbol 新增 `owner_name: Option<String>`<br>3. SYMBOL_QUERY 新增 init 捕获模式<br>4. classify_symbol 新增 init → Init<br>5. extract_cangjie_symbols_from_tree 支持 init 提取 |
| `crates/cangjie/src/graph.rs` | 1. 新增 `build_constructor_source_id()` helper<br>2. 新增 `source_id_to_symbol_node_id()` 映射<br>3. `emit_cangjie_reference_edges` 使用映射替代直接 source_id<br>4. `emit_synthetic_source_nodes` 只为未覆盖的 source ID 发 synthetic<br>5. `inspect_cangjie_project` 传递必要信息 |
| `crates/cangjie/tests/` | 新增 constructor extraction + coexistence + integrity tests |

## Forbidden Set

- 不删 `NodeKind::CallableSource`
- 不删 `emit_synthetic_source_nodes`
- 不改 `references.rs` 中的 `build_source_id()` 格式
- 不改 edge kind（Uses/Accesses/Modifies）
- 不做 Method symbol extraction
- 不改 GitNexus-RC / Tool / live repo
- 不新增依赖

## Acceptance Criteria

### Must Have
- [ ] `CangjieSymbolKind::Init` 枚举变体 + Display trait
- [ ] `CangjieSymbol.owner_name` 字段
- [ ] Symbol extraction 正确提取 class/struct body 中的 init（含 owner_name）
- [ ] Init symbol 有 Defines 边（SourceFile → init symbol）
- [ ] Constructor source_id → init symbol node_id 映射正确
- [ ] Synthetic nodes 只覆盖未被真实 init 覆盖的 Constructor source IDs
- [ ] Endpoint integrity 仍为 0 dangling
- [ ] `cargo fmt --check` + `cargo test` + `cargo test --features tree-sitter-cangjie` pass

### Should Have
- [ ] Before/after synthetic node 数量对比
- [ ] Constructor fixture（class init + struct init）
- [ ] Updated docs/plans/README.md

## Stop-line

- 如果 tree-sitter query 无法稳定捕获 init 节点 → 退回 docs-only + known limitation
- 如果 ID 映射逻辑导致 dangling → 回退到只保留 synthetic nodes
- 如果测试回归 → 立即 stop + 修复
