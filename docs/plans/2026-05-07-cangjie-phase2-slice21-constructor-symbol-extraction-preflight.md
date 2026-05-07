# Slice 21 Preflight — Constructor Symbol Extraction 语义核查

日期：2026-05-07

## 核心语义核查

### Q1: CallableSource / Synthetic Source Nodes 到底表示什么？

**回答：call-site source endpoint，不是"定义缺失"。**

Reference extraction 为每个引用构建 `source_id`，表示"引用发生在哪个作用域内"：
- `Constructor:/path/to/file:ClassName.init#arity` — 引用发生在 ClassName 的 init 块内
- `Method:/path/to/file:ClassName.methodName#arity` — 引用发生在 ClassName 的 methodName 方法内
- `Function:/path/to/file:funcName` — 引用发生在顶层 funcName 内

这些 source_id 用作 USES/ACCESSES/MODIFIES 边的 source endpoint。当 symbol extraction 未提取对应定义节点时，graph 中无对应 node，导致 dangling source。

### Q2: Constructor symbol extraction 应该生成什么？

**回答：Definition Symbol 节点。**

应新增 `CangjieSymbolKind::Init`（或复用 `Function` + `is_init` metadata），表示 class/struct 中的 `init` 定义。通过 `Defines` 边连接到所在 SourceFile。

### Q3: 是否能减少 synthetic nodes？

**回答：能减少，但不能完全消除。**

- 如果 symbol extraction 提取了 `init` 定义，`emit_cangjie_reference_edges` 可将 `Constructor:...` source_id 映射到真实 Symbol node，synthetic node 不再需要
- 但 Method 和 Function source_id 无法被 definition symbol 完全覆盖（method 的 source_id 格式含 arity，而 symbol ID 不含）
- **保留 synthetic nodes 作为 fallback**

### Q4: 是否改变 USES/ACCESSES/MODIFIES 的 source/target 语义？

**回答：不改变。**

edge 的 source 仍是 Constructor/Method/Function 节点，只是从 `NodeKind::CallableSource`（synthetic）升级为 `NodeKind::Symbol`（真实定义）。

### Q5: 是否可能把 "call source endpoint" 错误建模为 "constructor definition symbol"？

**回答：不会，两者 ID 格式不同。**

- Definition symbol ID: `sym:path:Init:ClassName.init`（由 `symbol_node_id` 生成）
- Call source ID: `Constructor:path:ClassName.init#arity`（由 `build_source_id` 生成）

需要在 `emit_cangjie_reference_edges` 中添加 source_id → symbol_node_id 映射逻辑。

### Q6: 是否需要保留 synthetic nodes？

**回答：是，保留为 fallback。**

- init 定义可覆盖大部分 `Constructor:*` source_id
- Method source_id 的 arity 信息无法被 symbol ID 表达，仍需 synthetic
- Function source_id 也可能无法完全覆盖
- **策略：先尝试匹配真实 Symbol node，未匹配的才发 synthetic**

## 方案决策

**方案 C（补充 + fallback）：**

1. 新增 `CangjieSymbolKind::Init`，提取 `init` 定义为 Symbol 节点
2. `emit_cangjie_reference_edges` 中，先尝试将 source_id 映射到真实 Symbol node
3. 未匹配的 source_id 继续由 `emit_synthetic_source_nodes` 处理
4. Synthetic nodes 数量预期大幅减少（Constructor 类从 ~646 降到接近 0）
5. **不删除 CallableSource / emit_synthetic_source_nodes**，它们是 production-safe fallback

## 实现范围

### Write set

| 文件 | 变更 |
|------|------|
| `crates/cangjie/src/extractors/symbol.rs` | 新增 `CangjieSymbolKind::Init`，SYMBOL_QUERY 添加 `initDefinition` 模式 |
| `crates/cangjie/src/extractors/references.rs` | 无变更 |
| `crates/cangjie/src/graph.rs` | `emit_cangjie_reference_edges` 添加 source_id → symbol node 映射；`emit_synthetic_source_nodes` 只为未匹配 source_id 发 synthetic |

### Forbidden set

- 不改 `CangjieSymbol` struct signature（只新增枚举变体）
- 不改 `CangjieReference` struct
- 不改 edge kind（Uses/Accesses/Modifies 不变）
- 不删 `NodeKind::CallableSource`
- 不删 `emit_synthetic_source_nodes`
- 不改 GitNexus-RC / Tool / live repo

### Stop-line

- 不做 method symbol extraction（Method source_id 含 arity，symbol ID 无法表达，留给未来）
- 不做 function body 内 function extraction（留作 future slice）
- 不做 constructor call resolution 增强（仅修 endpoint 映射）
- 不新增依赖

## 测试计划

1. `CangjieSymbolKind::Init` 枚举变体 + Display trait
2. Symbol extraction fixture：class init + struct init
3. Graph emission：init symbol 有 Defines 边
4. Reference edge source 映射：Constructor source_id → init Symbol node
5. Synthetic node 减少：production fixture 上 synthetic count 降低
6. Endpoint integrity 不回归：dangling source = 0, dangling target = 0
7. Production smoke 不回归

## 风险

- ⚠️ Cangjie grammar 对 `initDefinition` 的节点名可能不稳定（需 tree-sitter-cangjie 验证）
- ⚠️ 如果 grammar 不支持 `initDefinition`，此 slice 退回 docs-only，只记录 known limitation
- ⚠️ Method source_id 的 arity 后缀使映射不完美，synthetic nodes 不可能归零
