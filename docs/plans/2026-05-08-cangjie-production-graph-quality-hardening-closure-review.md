# Production Graph Quality Hardening — Closure Review

**Date:** 2026-05-08  
**Status:** Closure Review  
**Type:** Bug Fix / Quality Hardening  
**Parent:** Phase 2 autonomous advancement — Cangjie graph quality hardening on production fixture

---

## 问题

在 production fixture smoke 中发现两个 graph identity 问题：

### 1. Duplicate edge triples（已修复，Slice 20 follow-up）
同一 source file 中同一函数多次引用同一 struct（如多个 `let x: ArrayList<...>` 声明）会产生重复的 (kind, sourceId, targetId) edge triples，构成 multigraph 噪音。在 `inspect_cangjie_project()` 输出端做确定性去重（retain first occurrence）。

### 2. Duplicate Function symbol node IDs（本次修复，CRITICAL）
在 `json_parser` production fixture 中发现 16 个重复 Function node ID：
- `toString` 出现 6 次（每个 JSON 类型 class 各有一个）
- `asBool`, `asNumber`, `asString` 等 11 个方法各出现 2 次
- 共 12 个唯一 Function 名称产生 16 个重复 node ID

**根因**：这些 Function 实际是 class methods。symbol extraction 对 `functionDefinition` 节点统一创建 `CangjieSymbol { kind: Function, owner_name: None }`，导致同一个文件中的多个同名方法产生相同的 node ID `sym:<file>:Function:toString`。

Init symbols 在 Slice 21 已有 arity-based 唯一性修复，但 Function symbols 不受保护。

---

## 修复策略

### 方案：owner-based + arity 唯一性

1. **symbol.rs**: Function symbol extraction 时提取 `owner_name`（class/struct/enum/interface）和 `arity`（参数数量）
2. **symbol.rs**: 方法 name 格式化为 `Owner.funcName`
3. **graph.rs** `symbol_node_id()`: Function symbols 追加 `#arity` 后缀
4. **graph.rs** `emit_cangjie_reference_edges()`: 构建 Method source_id → Function symbol node_id 映射
5. **references.rs** `build_source_id()`: Function source IDs 追加 `#arity` 后缀

### Method → Function symbol 映射

与 Constructor → Init 映射对齐：
- Method source ID: `Method:<abs-path>:<Owner>.<funcName>#<arity>`
- Function symbol node ID: `sym:<rel-path>:Function:<Owner>.<funcName>#<arity>`
- Method source ID 被 resolve 为 Function symbol node ID，消除 Method synthetic nodes

### 为什么不使用 start_line
- 与 Constructor/Init 的 arity-based 格式不一致
- owner_name 提供语义信息（方法属于哪个 class），对齐 Method source ID 格式
- arity 可区分同一 class 中的同名重载方法

---

## 实现变更

### `crates/cangjie/src/extractors/symbol.rs`

1. `extract_owner_name()`: 新增 `enumDefinition`/`interfaceDefinition` 支持
2. `count_init_params()` → `count_params()`: 重命名为通用函数
3. Function symbol 提取：调用 `extract_owner_name()` + `count_params()`，填充 `owner_name`/`arity`
4. 方法 name 格式：`Owner.funcName`（owner 存在时）

### `crates/cangjie/src/graph.rs`

1. `symbol_node_id()`: Function symbols 追加 `#arity` 后缀（与 Init 格式一致）
2. `emit_cangjie_reference_edges()`: 新增 `method_to_symbol_id` 映射
3. `resolve_source_id()`: 处理 `Method:` 前缀 source ID 映射
4. `extract_function_label()`: 剥离 `#arity` 后缀显示干净 label

### `crates/cangjie/src/extractors/references.rs`

1. `build_source_id()`: Function source ID 追加 `#arity` 后缀

### Node ID 最终格式

```
# Top-level function
sym:src/main.cj:Function:escapeJsonString#1

# Method (inside class)
sym:src/json_value.cj:Function:JsonNull.toString#0
sym:src/json_value.cj:Function:JsonBool.toString#0
sym:src/json_value.cj:Function:JsonNum.toString#0
sym:src/json_value.cj:Function:JsonStr.toString#0
sym:src/json_value.cj:Function:JsonArr.toString#0
sym:src/json_value.cj:Function:JsonObj.toString#0
```

---

## 验证结果

- `cargo fmt --check`: ✅ clean
- `cargo test`: ✅ all pass (without feature)
- `cargo test --features tree-sitter-cangjie`: ✅ all pass (287+ tests, 0 fail)
- `cargo test --features tree-sitter-cangjie --test constructor_extraction -- --nocapture`: ✅ 12/12 pass
- `cargo test --features tree-sitter-cangjie --test endpoint_integrity -- --nocapture`: ✅ 12/12 pass
- `cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`: ✅ all assertions pass
- `git diff --check`: ✅ clean

### Before/After — json_parser production fixture

| Metric | Before | After |
|--------|--------|-------|
| Duplicate node IDs | 16 | **0** |
| Method synthetic nodes | ~16 | **0** |
| Dangling source edges | 0 | 0 |
| Dangling target edges | 0 | 0 |
| Output deterministic | — | true |
| Init symbols with #arity | 8/8 | 8/8 |
| Constructor synthetic nodes | 0 | 0 |

### Before/After — 4 production targets summary

| Target | Duplicates Before | Duplicates After | Deterministic |
|--------|------------------|------------------|---------------|
| cjgui (GitNexus-Index) | 0 | 0 | true |
| cjgui (cangjie) | 0 | 0 | true |
| web_framework | 0 | 0 | true |
| json_parser | **16** | **0** | true |

---

## 禁止事项遵守

- ✅ 不改 GitNexus-RC
- ✅ 不改 GitNexus-RC-Tool
- ✅ 不改 live repo
- ✅ 不做 destructive git 操作
- ✅ 不新增依赖
- ✅ 不做 method dispatch
- ✅ 不做 type inference
- ✅ 不做 overload resolution
- ✅ 不做 macro expansion
- ✅ 不开启新 slice

---

## Exit Criteria

- ✅ `cargo fmt --check` pass
- ✅ `cargo test` pass
- ✅ `cargo test --features tree-sitter-cangjie` pass
- ✅ Production smoke all 4 targets pass
- ✅ Duplicate node IDs = 0 (all targets)
- ✅ Duplicate edge triples = 0 (all targets)
- ✅ Endpoint integrity 0 dangling (all targets)
- ✅ Output deterministic (all targets)
- ✅ Init symbols all have #arity suffix
- ✅ Constructor synthetic nodes = 0
- ✅ Method synthetic nodes = 0 (json_parser, web_framework)
- ✅ Closure review 完成
- ⏳ Commit + push（进行中）

**Hardening 状态：** ✅ 完成
