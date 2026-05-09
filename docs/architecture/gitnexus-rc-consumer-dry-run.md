# GitNexus-RC 消费侧兼容性 Dry-Run 报告

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Active
> **Stop-line：** 只读审计，不修改 GitNexus-RC / GitNexus-RC-Tool / live repo

---

## 目的

验证 Rust-core `--format gitnexus-rc` bridge JSON 与 GitNexus-RC 消费侧（Web UI、LLM Tools、ingestion pipeline、graph adapter）预期之间的兼容性。本轮仅做只读审计，记录匹配/差异，不修改 GitNexus-RC。

---

## 一、GitNexus-RC 消费侧入口清单

### 1.1 已审计文件

| 文件 | 角色 | 消费内容 |
|------|------|---------|
| `gitnexus-shared/src/graph/types.ts` | 类型定义源头 | `GraphNode`, `GraphRelationship`, `NodeLabel` (40+), `RelationshipType` (24), `KnowledgeGraph` |
| `gitnexus-shared/src/lbug/schema-constants.ts` | DB schema 常量 | `NODE_TABLES` (34), `REL_TYPES` (22), `CodeRelation` 单表 |
| `gitnexus-web/src/lib/graph-adapter.ts` | Web UI Sigma.js 适配 | `knowledgeGraphToGraphology()`: nodes (id/label/properties), relationships (sourceId/targetId/type) |
| `gitnexus-web/src/lib/constants.ts` | Web UI 常量 | 40+ `NodeLabel`, 11 `EdgeType`, 颜色/尺寸/边样式 |
| `gitnexus-web/src/core/llm/tools.ts` | Graph RAG Tools (7个) | Cypher 查询: NODE_TABLES, REL_TYPES, CodeRelation type/confidence/reason |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/types.ts` | Rust-core adapter 类型 | `RustCoreGraphOutput` (8 node labels + 9 edge types), validation rules |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/map-to-gitnexus.ts` | 节点/边映射逻辑 | label→NodeLabel dispatch, edge type→RelationshipType mapping, ID 重构, confidence/reason 注入 |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/validate.ts` | 合约验证器 | schema version check, node/edge label 白名单, duplicate detection, stats consistency |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/index.ts` | 入口 | `loadRustCoreGraph()`: JSON→validate→map→KnowledgeGraph |
| `gitnexus/src/core/graph/types.ts` | KnowledgeGraph 接口 | `addNode()`, `addRelationship()`, `iterNodes()`, `iterRelationshipsByType()` |
| `gitnexus/src/core/graph/graph.ts` | KnowledgeGraph 实现 | Map-based 存储, 边 type 索引, 反向邻接索引 |

### 1.2 消费路径

```
Rust-core CLI JSON → (--format gitnexus-rc) → stdout
                                                  ↓
GitNexus-RC adapter: loadRustCoreGraph() → validate → mapToKnowledgeGraph → KnowledgeGraph
                                                  ↓
                                  ┌───────────────┼──────────────────┐
                                  ↓               ↓                  ↓
                           LadybugDB        Web Sigma.js       LLM Cypher
                           (NODE_TABLES,    (graph-adapter)    (tools.ts)
                            REL_TYPES)
```

**注意：** 当前 GitNexus-RC adapter (`rust-core-graph-adapter/`) 消费的是 Rust-core **原始** GraphOutput 格式（flat `nodes[]` + `edges[]`，source/target 端点），而不是 bridge JSON 格式。Bridge JSON 是新格式，尚未被 GitNexus-RC adapter 消费。

---

## 二、Rust-core Bridge JSON 当前结构

### 2.1 顶层字段

```json
{
  "schemaVersion": "0.3.0",
  "generatedAt": "2026-05-09T...",
  "language": "rust",
  "root": "/path/to/project",
  "repository": { "id": "repo:...", "path": "..." },
  "packages": [{ "id": "...", "name": "...", "manifestPath": "..." }],
  "sourceFiles": [{ "id": "file:...", "path": "...", "packageId": "..." }],
  "symbols": [{ "id": "sym:...", "name": "...", "kind": "...", ... }],
  "edges": { "calls": [...], "defines": [...], "uses": [...], ... },
  "diagnostics": [...],
  "stats": { "nodeCount": N, "edgeCount": N, ... }
}
```

### 2.2 Node 字段

| Bridge 字段 | 类型 | 来源 |
|------------|------|------|
| `id` | string | Rust/Cangjie graph node ID |
| `name` | string | 从 properties 提取 |
| `kind` | string | Rust: properties.kind (默认 "symbol"), Cangjie: properties.kind |
| `fileId` | string? | Rust: properties.fileId, Cangjie: null |
| `parentId` | string? | Rust: properties.parentId, Cangjie: null |
| `packageId` | string? | Rust: 通过 edge traversal 解析, Cangjie: properties.packageId |
| `manifestPath` | string | 仅 packages |
| `path` | string | 仅 sourceFiles (从 sourcePath 字段提取) |
| `properties` | object | 其余属性 |

### 2.3 Edge 字段

| Bridge 字段 | 类型 | 说明 |
|------------|------|------|
| `sourceId` | string | 归一化端点（Rust: source→sourceId, Cangjie: sourceId→sourceId） |
| `targetId` | string | 归一化端点（Rust: target→targetId, Cangjie: targetId→targetId） |
| `kind` | string | Rust graph `type` 字段值（大写，如 CALLS/DEFINES） |

### 2.4 Edge 分组

edges 按语义类别分组为: `calls`, `defines`, `uses`, `accesses`, `designations`, `imports`, `contains`, `owns`, `annotates`, `other`

---

## 三、字段匹配表

### 3.1 顶层结构匹配

| Bridge JSON 字段 | GitNexus-RC 预期 | 匹配状态 | 说明 |
|-----------------|-----------------|---------|------|
| `schemaVersion` | `schemaVersion` | ✅ 兼容 | 字符串格式一致 |
| `generatedAt` | `generatedAt` | ✅ 兼容 | ISO 8601 |
| `language` | N/A (adapter 不消费) | ✅ 额外但无害 | |
| `root` | N/A | ✅ 额外但无害 | |
| `repository` (single obj) | nodes[] 含 Project label | ⚠️ 需 adapter | Bridge 是单体对象，RC 是 node |
| `packages[]` | nodes[] 含 Package label | ⚠️ 需 adapter | Bridge 含 target nodes（RC adapter 为 metadata-only） |
| `sourceFiles[]` | nodes[] 含 File label | ⚠️ 需 adapter | |
| `symbols[]` | nodes[] 含 Function/Class/... label | ⚠️ 需 adapter | |
| `edges` (grouped) | relationships[] (flat) | ⚠️ 需 adapter | Bridge 分组，RC 扁平 |
| `diagnostics[]` | N/A (RC adapter 不消费) | ✅ 兼容 | RC 仅统计，不映射为 node |
| `stats` | stats (loose validation) | ✅ 兼容 | 字段一致 |

### 3.2 Node 字段匹配

| Bridge Node 字段 | GitNexus-RC GraphNode | 匹配状态 | 说明 |
|-----------------|----------------------|---------|------|
| `id` | `id` | ✅ 兼容 | 但 ID 格式不同（见 §五） |
| `kind` (string) | `label` (NodeLabel enum) | ❌ 不兼容 | Bridge 用 "symbol"/"package" 等通用 label，RC 用 "Function"/"Struct" 等具体 label |
| `name` | `properties.name` | ✅ 兼容（需 adapter） | Bridge 顶层字段，RC 在 properties 内 |
| `path` (sourceFiles) | `properties.filePath` | ✅ 兼容（需 adapter） | |
| `manifestPath` (packages) | `properties.filePath` | ⚠️ 语义差异 | RC adapter 将 manifestPath 映射为 filePath |
| `fileId` | N/A | ✅ 额外但无害 | RC 通过 edge 隐式表达 |
| `parentId` | N/A | ✅ 额外但无害 | RC 用 HAS_PARENT/MEMBER_OF edge 表达 |
| `properties` (extra) | `properties` | ⚠️ 部分兼容 | 字段名差异：symbolKind vs kind, sourcePath vs filePath |

### 3.3 Edge 字段匹配

| Bridge Edge 字段 | GitNexus-RC GraphRelationship | 匹配状态 | 说明 |
|-----------------|------------------------------|---------|------|
| `sourceId` | `sourceId` | ✅ 兼容 | Bridge 已归一化 |
| `targetId` | `targetId` | ✅ 兼容 | Bridge 已归一化 |
| `kind` | `type` (RelationshipType) | ⚠️ 字段名 + 值差异 | Bridge 用 "kind"，RC 用 "type"；值也不同（见 §四） |
| N/A | `id` (rel ID) | ❌ 缺失 | RC 每条 edge 需要唯一 ID |
| N/A | `confidence` (0-1) | ❌ 缺失 | RC 边需要置信度 |
| N/A | `reason` (string) | ❌ 缺失 | RC 边需要解析原因 |
| `properties` | N/A | ✅ 额外但无害 | RC 不消费 edge properties |

---

## 四、Node Kind / Edge Kind 映射表

### 4.1 Rust Node Label → GitNexus-RC NodeLabel

| Bridge kind (from label) | Bridge properties.symbolKind | GitNexus-RC NodeLabel | 匹配 |
|--------------------------|------------------------------|----------------------|------|
| `repository` (via repo object) | — | `Project` | ✅ adapter 映射 |
| `package` | — | `Package` | ✅ adapter 映射 |
| `target` | — | N/A (metadata-only) | ⚠️ Bridge 含 target，RC 不映射 |
| `source-file` (via sourceFiles) | — | `File` | ✅ adapter 映射 |
| `module` (跳过) | — | `Module` | ❌ Bridge 跳过 module nodes |
| `symbol` | `struct` | `Struct` | ✅ adapter mapping |
| `symbol` | `enum` | `Enum` | ✅ adapter mapping |
| `symbol` | `trait` | `Trait` | ✅ adapter mapping |
| `symbol` | `impl-block` | `Impl` | ✅ adapter mapping |
| `symbol` | `function` | `Function` | ✅ adapter mapping |
| `symbol` | `method` | `Method` | ✅ adapter mapping |
| `symbol` | `associated-function` | `Method` | ✅ adapter mapping |
| `symbol` | `const` | `Const` | ✅ adapter mapping |
| `symbol` | `static` | `Static` | ✅ adapter mapping |
| `symbol` | `macro-definition` | `Macro` | ✅ adapter mapping |
| `symbol` | `type-alias` | `TypeAlias` | ✅ adapter mapping |
| `symbol` | `module` | `Module` | ✅ adapter mapping |
| `symbol` | `enum-variant` | `CodeElement` | ⚠️ RC 无 EnumVariant label，fallback |
| `diagnostic` (skipped) | — | `Diagnostic` | ❌ Bridge 跳过 diagnostic nodes |

### 4.2 Cangjie Node Kind → GitNexus-RC NodeLabel

| Bridge kind (from Cangjie) | GitNexus-RC NodeLabel | 匹配 |
|---------------------------|----------------------|------|
| `repository` | `Project` | ✅ |
| `package` | `Package` | ✅ |
| `sourceFile` | `File` | ✅ |
| `symbol` (kind=Function) | `Function` | ✅ |
| `symbol` (kind=Class) | `Class` | ✅ |
| `symbol` (kind=Struct) | `Struct` | ✅ |
| `symbol` (kind=Enum) | `Enum` | ✅ |
| `symbol` (kind=Interface) | `Interface` | ✅ |
| `symbol` (kind=TypeAlias) | `TypeAlias` | ✅ |
| `symbol` (kind=Macro) | `Macro` | ✅ |
| `symbol` (kind=Init) | `Constructor` | ✅ |
| `callableSource` (synthetic) | `Function` | ⚠️ 合成节点，语义不同 |

### 4.3 Edge Kind 映射表

| Bridge edge.kind (Rust) | Bridge edge.kind (Cangjie) | Bridge 分组 | GitNexus-RC rel.type | RC adapter 映射 | 匹配 |
|--------------------------|---------------------------|------------|---------------------|----------------|------|
| `CALLS` | — | `calls` | `CALLS` | `CALLS` | ✅ |
| `DEFINES` | `defines` | `defines` | `DEFINES` | `DEFINES` | ✅ |
| — | `uses` | `uses` | `USES` | N/A (RC 无) | ⚠️ Cangjie 有, RC REL_TYPES 有但 adapter 不用 |
| `ACCESSES` | `accesses` | `accesses` | `ACCESSES` | N/A (RC adapter skip) | ⚠️ |
| `DESIGNATION` | — | `designations` | `ANNOTATES`? | N/A (RC adapter skip) | ❌ RC 无 DESIGNATION type |
| — | `imports` | `imports` | `IMPORTS` | N/A (RC adapter skip) | ⚠️ RC REL_TYPES 有 |
| `CONTAINS_PACKAGE` | `containsPackage` | `contains` | `CONTAINS` | `CONTAINS` | ✅ |
| `CONTAINS_WORKSPACE` | `containsWorkspace` | `contains` | N/A | N/A (metadata-only) | ⚠️ RC adapter skip |
| `HAS_TARGET` | — | `contains` | N/A | N/A (metadata-only) | ⚠️ |
| `OWNS_SOURCE` | `ownsSource` | `owns` | `CONTAINS` | `CONTAINS` | ✅ |
| `HAS_PARENT` | `hasParent` | `other` | `MEMBER_OF` | `MEMBER_OF` | ✅ |
| `RESOLVES_TO` | `resolvesTo` | `other` | N/A | N/A (metadata-only) | ⚠️ |
| `ANNOTATES` | `annotates` | `annotates` | `ANNOTATES` | N/A (metadata-only) | ⚠️ |
| — | `modifies` | `other` | `MODIFIES` | N/A (RC adapter skip) | ⚠️ RC REL_TYPES 有 |

### 4.4 GitNexus-RC 已有但 Rust-core 缺失的 Edge Types

| GitNexus-RC type | Web UI 支持 | LLM Tools 使用 | Rust-core 状态 |
|------------------|------------|---------------|---------------|
| `EXTENDS` | ✅ 颜色/样式 | ✅ (impact default, cypher) | ❌ 无 |
| `IMPLEMENTS` | ✅ 颜色/样式 | ✅ (impact default, cypher) | ❌ 无 |
| `IMPORTS` | ✅ 层次关系 | ✅ (impact default, cypher) | Cangjie ✅ / Rust ❌ |
| `MEMBER_OF` | ✅ 颜色/样式 | ❌ | Rust: HAS_PARENT 映射 |
| `STEP_IN_PROCESS` | ✅ 颜色/样式 | ✅ (explore/overview cypher) | ❌ 无 |
| `MODIFIES` | ✅ 颜色/样式 | ❌ | Cangjie: 在 other 分组 |
| `HAS_METHOD` | ❌ | ❌ | ❌ 无 |
| `HAS_PROPERTY` | ❌ | ❌ | ❌ 无 |

---

## 五、已知风险与差异

### 5.1 ID 命名差异（中等风险）

| 来源 | ID 格式 | 示例 |
|------|---------|------|
| Rust-core bridge | `repo:...`, `pkg:...`, `file:...`, `symbol:...`, `target:...` | `symbol:portable-smoke::crate::Calculator` |
| GitNexus-RC adapter 转换后 | `{filePath}:{NodeLabel}:{name}` | `src/lib.rs:Struct:Calculator` |

**影响：** Bridge JSON 的 ID 与 GitNexus-RC KnowledgeGraph 的 ID 不能直接互通。如果消费侧直接用 bridge JSON ID 查询 knowledge graph，会找不到对应节点。

**缓解：** Rust-core bridge JSON 文档 §八 已声明 Node ID 格式不保证跨版本稳定。消费侧应通过 adapter 转换，不应直接依赖 raw ID。

### 5.2 Node kind/label 差异（高风险）

**问题：** Bridge JSON 中 symbol 节点的 `kind` 字段为通用 "symbol"（来自 Rust graph node label），具体符号类型在 `properties.symbolKind` 中。GitNexus-RC 的 `GraphNode.label` 要求具体类型（如 "Function"）。

**当前 bridge 输出：**
```json
{ "id": "symbol:...", "name": "Calculator", "kind": "symbol",
  "properties": { "symbolKind": "struct", ... } }
```

**GitNexus-RC 期望：**
```json
{ "id": "src/lib.rs:Struct:Calculator", "label": "Struct",
  "properties": { "name": "Calculator", "filePath": "src/lib.rs", ... } }
```

**修复方向：** Bridge 格式可在 `symbols[].kind` 中填入 `properties.symbolKind` 的值（而非通用的 "symbol"），或新增 `label` 字段直接对齐 GitNexus-RC 的 NodeLabel 枚举。

### 5.3 packages 含 target 节点（低风险）

**问题：** Rust bridge JSON 将 `target` 节点（lib/bin）也放入 `packages` 数组。GitNexus-RC adapter 将 target 视为 metadata-only，不映射为 Package node。

**影响：** `stats.packageCount` 包含 target 节点，与 GitNexus-RC 的 package count 语义不同。消费侧不应跨格式比较 packageCount。

### 5.4 Edge confidence/reason 缺失（中等风险）

**问题：** Bridge JSON 边不含 `confidence` 和 `reason` 字段。GitNexus-RC 的 GraphRelationship 要求这两个字段（CALLS 边 confidence 0.5-0.9，structural edges 0.9-1.0）。

**影响：**
- Web UI 不消费 confidence/reason（仅边样式）
- LLM Tools 的 impact/explore 工具会显示 confidence（如 "confidence < 80% = fuzzy"）
- Cypher 查询可过滤 `r.confidence > 0.8`

**修复方向：** Bridge 格式可为每条边添加 confidence 和 reason 字段。Rust-core 原始 graph output 已有 confidence/reason 在 edge properties 中，bridge 转换时丢失了这些信息。

### 5.5 Edge type 命名差异（中等风险）

**问题：** Bridge edge `kind` 使用 Rust/Cangjie 原始 edge type 名称（大写，如 `CALLS`、`DEFINES`）。GitNexus-RC 使用 `RelationshipType` 枚举（含 `USES`、`IMPORTS`、`MODIFIES` 等额外类型）。

**影响：** 以下 GitNexus-RC edge types 在 Rust-core bridge 中不存在或分组不同：
- `EXTENDS`, `IMPLEMENTS`: Rust-core 无此概念
- `IMPORTS`: Cangjie 有，Rust 无
- `MODIFIES`: Cangjie 有（在 other 分组），Rust 无
- `MEMBER_OF`: Rust HAS_PARENT 映射到 other 分组，非 explicit
- `STEP_IN_PROCESS`: 两个语言都无

### 5.6 sourceFiles/packageId 差异（低风险）

**问题：** Rust source-files 的 `packageId` 通过 `OWNS_SOURCE → HAS_TARGET` edge traversal 两跳解析，复杂 workspace 结构可能缺失。Cangjie source-files 直接有 `packageId`。

**影响：** 部分 Rust source-file 的 `packageId` 可能为 null。消费侧不应依赖所有 source-file 都有 packageId。

### 5.7 diagnostics 差异（低风险）

**问题：** Rust bridge JSON 含 diagnostics 数组，Cangjie bridge JSON 当前可能为空。GitNexus-RC adapter 不将 diagnostic 映射为 KnowledgeGraph node（仅统计）。

**影响：** Diagnostic nodes 不是一等节点（在 GitNexus-RC 中），但 bridge JSON 保留诊断数据。未来如果 RC 需要消费诊断，需要扩展 adapter。

### 5.8 Rust/Cangjie 双语言 schema 差异（中等风险）

**问题：** Rust 和 Cangjie 的 graph 结构不同（Rust 有 workspace/target/module 概念，Cangjie 无）。Bridge JSON 对两种语言产生不同的节点/边组合。

**影响：** 消费侧不能假设所有语言的 bridge JSON 字段完全相同。需按 `language` 字段区分处理。

---

## 六、下一步建议

### 6.1 Rust-core 内部可修（不改 GitNexus-RC）

| 修复项 | 优先级 | 说明 |
|--------|--------|------|
| Symbol `kind` 填入具体类型 | **高** | 将 `kind: "symbol"` 改为 `kind: "Function"` / `kind: "Struct"` 等 |
| Edge 添加 `confidence` + `reason` | **中** | 从原始 edge properties 提取，bridge 转换时保留 |
| Edge `kind` 添加分组标签 | **低** | 可选：增加 `group` 字段标明边属于 calls/defines/contains 等分组 |
| Symbol `packageId` 补齐 | **低** | 通过 file→package edge traversal 解析 |
| Diagnostic nodes 纳入 symbols | **低** | 可选：将 diagnostic 作为一等 symbol 输出 |

### 6.2 需要 GitNexus-RC adapter（跨仓改动，当前不做）

| 适配项 | 说明 |
|--------|------|
| Bridge JSON → KnowledgeGraph 新 adapter | 现有 adapter 消费原始 GraphOutput，需新增 bridge format 路径 |
| Node ID 格式转换 | Bridge 的 `repo:`/`pkg:`/`symbol:` 前缀 → RC 的 `{path}:{Label}:{name}` |
| Edge type 映射表扩展 | 新增 USES/IMPORTS/MODIFIES 等 Cangjie edge types |
| Diagnostic → RC node 映射 | 当前 RC adapter 仅统计，未来可映射为 Diagnostic node |

### 6.3 需要前端消费侧调整（跨仓改动，当前不做）

| 调整项 | 说明 |
|--------|------|
| 新增 Rust/Cangjie 语言 NodeLabel 类型 | Rust: EnumVariant, ImplBlock; Cangjie: Init, CallableSource |
| 新增 EdgeType | DESIGNATION（Rust 专属） |
| 调整 color/size 表 | 为新增 NodeLabel 配置颜色和大小 |

### 6.4 暂不做

| 项目 | 原因 |
|------|------|
| `EXTENDS` / `IMPLEMENTS` edge | Rust-core 不做 type inference，无法解析继承关系 |
| `STEP_IN_PROCESS` edge | Rust-core 不做 process/flow 分析 |
| `HAS_METHOD` / `HAS_PROPERTY` edge | Rust-core 当前不提取 method/property 列表 |
| Node ID 格式标准化 | 与 GitNexus-RC 维护者协商后再定 |
| Schema 向前兼容 | 待 consumer contract 稳定后 |

---

## 七、GitNexus-RC 消费侧已知消费点（完整索引）

```
gitnexus-shared/src/graph/types.ts
  └─ NodeLabel (40+), RelationshipType (24), GraphNode, GraphRelationship

gitnexus-shared/src/lbug/schema-constants.ts
  └─ NODE_TABLES (34), REL_TYPES (22), REL_TABLE_NAME, EMBEDDING_TABLE_NAME

gitnexus-web/src/lib/graph-adapter.ts
  └─ knowledgeGraphToGraphology(): knowledgeGraph.nodes/relationships → Sigma.js
  └─ 消费: node.id, node.label, node.properties (name/filePath/startLine/endLine/severity/message/source/rule)
  └─ 消费: rel.sourceId, rel.targetId, rel.type

gitnexus-web/src/lib/constants.ts
  └─ NODE_COLORS, NODE_SIZES (per NodeLabel), EdgeType (11), EDGE_STYLES

gitnexus-web/src/core/llm/tools.ts
  └─ search: NODE_TABLES, 结果含 name/label/filePath/connections (type+confidence)
  └─ cypher: NODE_TABLES + REL_TYPES, CodeRelation (type, confidence, reason)
  └─ impact: default relTypes = CALLS/IMPORTS/EXTENDS/IMPLEMENTS, minConfidence=0.7
  └─ overview: Community/Process/CodeRelation queries
  └─ explore: symbol/cluster/process + connections (type+confidence)

gitnexus/src/core/ingestion/rust-core-graph-adapter/
  └─ validate.ts: schemaVersion v0.2, 8 node labels, 9 edge types, duplicate check
  └─ map-to-gitnexus.ts: label→NodeLabel, edge type→RelationshipType, ID build, confidence/reason
  └─ types.ts: ALLOWED_NODE_LABELS (8), ALLOWED_EDGE_TYPES (9), WRITABLE_EDGE_TYPES (5)
```

---

## 八、结论

### 8.1 总体兼容性：部分兼容（需 adapter 层）

Rust-core `--format gitnexus-rc` bridge JSON 与 GitNexus-RC 消费侧在**结构层面基本对齐**（有顶层 repository/packages/sourceFiles/symbols/edges/diagnostics/stats），但在**字段语义层面存在差异**：

1. **Node kind 字段不匹配**（高风险）：bridge 用通用 label，RC 需具体 NodeLabel
2. **Edge type 命名不完全对齐**（中等风险）：bridge 保留原始 type 名称，RC 有不同枚举值
3. **confidence/reason 缺失**（中等风险）：bridge 边无这两个字段
4. **ID 格式不同**（中等风险）：bridge 和 RC 用不同 ID 策略
5. **packages 语义差异**（低风险）：bridge 含 target nodes
6. **Rust/Cangjie 差异**（中等风险）：两种语言产出不同节点/边组合

### 8.2 可以直接消费的字段

- `repository.id` / `repository.path`
- `packages[].id` / `name` / `manifestPath`
- `sourceFiles[].id` / `path`
- `symbols[].id` / `name` / `fileId` / `parentId`
- `edges.*[].sourceId` / `targetId`（端点已归一化）
- `stats.*`（所有统计字段）
- `diagnostics[]`（结构兼容）
- `schemaVersion` / `generatedAt` / `language` / `root`

### 8.3 需要 adapter 的字段

- `symbols[].kind`（需映射为具体 NodeLabel）
- `edges.*[].kind`（需映射为 RelationshipType）
- Node/Edge ID（需格式转换）
- Edge confidence/reason（需补全）

### 8.4 审计完整性

已读 GitNexus-RC 文件：11 个核心文件，覆盖全消费链（shared types → adapter → web UI → LLM tools → ingestion pipeline）。未发现未知消费点。

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.0.0 | 初始 dry-run：GitNexus-RC 消费侧完整审计、字段匹配表、node/edge kind 映射表、已知风险分类、下一步建议 |
