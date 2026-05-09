# GitNexus-RC 消费侧兼容性 Dry-Run 报告

> **日期：** 2026-05-09
> **版本：** v1.4.0
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
| `gitnexus-web/src/lib/constants.ts` | Web UI 常量 | 40+ `NodeLabel`, 12 `EdgeType`, 颜色/尺寸/边样式 |
| `gitnexus-web/src/core/llm/tools.ts` | Graph RAG Tools (7个) | Cypher 查询: NODE_TABLES, REL_TYPES, CodeRelation type/confidence/reason |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/types.ts` | 旧 adapter 类型 | `RustCoreGraphOutput` (8 node labels + 9 edge types), validation rules |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/map-to-gitnexus.ts` | 旧 adapter 映射 | label→NodeLabel dispatch, edge type→RelationshipType mapping, ID 重构 |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/validate.ts` | 旧 adapter 验证 | schema v0.2.0 检查, 8 node/9 edge 白名单, USES/IMPORTS/IMPLEMENTS 禁止 |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/index.ts` | 旧 adapter 入口 | `loadRustCoreGraph()`: JSON→validate→map→KnowledgeGraph |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/types.ts` | **Bridge adapter 类型** | `BridgeGraphOutput` (14顶层字段), 13 Rust + 9 Cangjie symbol kind, 21 edge kind, 10 edge 分组 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/validate.ts` | **Bridge adapter 验证** | 10 条验证规则：顶层字段、language、symbol kind 按语言白名单、duplicate ID、dangling 端点、edge kind 白名单、端点字段名 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/map-to-gitnexus.ts` | **Bridge adapter 映射** | 6-phase pipeline: repo→pkg→file→sym→flat edges→diagnostics；24 种 kind→label，24 种 kind→rel type；confidence/reason 策略 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/index.ts` | **Bridge adapter 入口** | `loadRustCoreBridgeGraph()`: JSON→validate→map→KnowledgeGraph |
| `gitnexus/src/core/graph/types.ts` | KnowledgeGraph 接口 | `addNode()`, `addRelationship()`, `iterNodes()`, `iterRelationshipsByType()` |
| `gitnexus/src/core/graph/graph.ts` | KnowledgeGraph 实现 | Map-based 存储, 边 type 索引, 反向邻接索引 |

### 1.2 消费路径

```
Rust-core CLI JSON → (--format gitnexus-rc) → stdout
                                                  ↓
GitNexus-RC bridge adapter: loadRustCoreBridgeGraph() → validate → mapBridgeGraphToKnowledgeGraph → KnowledgeGraph
                                                  ↓
                                  ┌───────────────┼──────────────────┐
                                  ↓               ↓                  ↓
                           LadybugDB        Web Sigma.js       LLM Cypher
                           (NODE_TABLES,    (graph-adapter)    (tools.ts)
                            REL_TYPES)
```

**两条 adapter 路径：**
- `rust-core-graph-adapter/`（旧）：消费 Rust-core **原始** GraphOutput 格式（flat `nodes[]` + `edges[]`，`source`/`target` 端点字段），schema v0.2.0
- `rust-core-bridge-adapter/`（新）：消费 `--format gitnexus-rc` bridge JSON（grouped edges by 10 categories，`sourceId`/`targetId` 归一化端点，symbol kind 具体化），schema v0.3.0

两条路径独立，互不干扰。本报告主要关注 bridge format 的兼容性。

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
| N/A | `id` (rel ID) | ❌ 缺失 | RC 每条 edge 需要唯一 ID，bridge 边无此字段 |
| `confidence` (Option<f64>) | `confidence` (0-1) | ✅ 已补齐（Rust） / ⚠️ Cangjie 为 null | Rust 语义边已从原始 edge properties 提升到顶层；Cangjie 源数据不提供 |
| `reason` (Option<String>) | `reason` (string) | ✅ 已补齐（Rust） / ⚠️ Cangjie 为 null | 同上，Rust 已透传，Cangjie 源数据不提供 |
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

### 5.4 Edge confidence/reason 缺失 → ✅ 已修复（2026-05-09）

**修复：** Rust bridge 边现在从原始 graph edge properties 中提取 `confidence`（f64）和 `reason`（string），提升到 `BridgeEdge` 顶层字段。`cangjie_bridge` 源数据不提供这些字段，因此 confidence/reason 为 `None`（Cangjie adapter 接入时需决策默认值或从其他来源补充）。

**残留差异：** Cangjie bridge 边的 confidence/reason 均为 null，RC adapter 需处理 null 情况（设为默认 1.0 或从 edge kind 推断）。

**影响（仅 Cangjie）：**
- Web UI 不消费 confidence/reason（仅边样式）
- LLM Tools 的 impact/explore 工具会显示 confidence（如 "confidence < 80% = fuzzy"）
- Cypher 查询可过滤 `r.confidence > 0.8`

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

| 修复项 | 优先级 | 状态 |
|--------|--------|------|
| Symbol `kind` 填入具体类型 | **高** | ✅ 已修复（`kind` 从 Rust `symbolKind` 或 Cangjie `kind` 属性提取，不再输出通用 "symbol"） |
| Edge 添加 `confidence` + `reason` | **中** | ✅ 已修复（Rust bridge 从原始 edge properties 提升到顶层，Cangjie 源数据不提供故为 null） |
| Edge `kind` 添加分组标签 | **低** | 不适用（bridge edges 已按语义分组为 `calls`/`defines`/`uses`/... 顶层数组） |
| Symbol `packageId` 补齐 | **低** | 未做（需 RC adapter 决策是否必须） |
| Diagnostic nodes 纳入 symbols | **低** | 未做（RC adapter 仅统计 diagnostic，不映射为 node） |

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

Rust-core `--format gitnexus-rc` bridge JSON 与 GitNexus-RC 消费侧在**结构层面基本对齐**（有顶层 repository/packages/sourceFiles/symbols/edges/diagnostics/stats）。后续 follow-up 已在 Rust-core bridge 侧补齐两项可本仓闭合的问题：`symbols[].kind` 不再输出通用 `"symbol"`，语义边的 `confidence` / `reason` 已提升到 edge 顶层并由 `bridge_roundtrip` 测试覆盖。

剩余差异集中在 **GitNexus-RC adapter 层**，不是 Rust-core bridge JSON 的自洽性问题：

1. **Node kind 语义映射**（中等风险）：bridge 已输出具体 kind，但 RC 仍需将 Rust/Cangjie kind 映射到自己的 NodeLabel 集合
2. **Edge type 命名不完全对齐**（中等风险）：bridge 保留原始 type 名称，RC 有不同枚举值，需 adapter 映射
3. **ID 格式不同**（中等风险）：bridge 和 RC 用不同 ID 策略，不能直接互查
4. **packages 语义差异**（低风险）：bridge 含 target nodes，RC adapter 当前视 target 为 metadata-only
5. **Rust/Cangjie 差异**（中等风险）：两种语言产出不同节点/边组合，消费侧需按 `language` 分支处理

### 8.2 可以直接消费的字段

- `repository.id` / `repository.path`
- `packages[].id` / `name` / `manifestPath`
- `sourceFiles[].id` / `path`
- `symbols[].id` / `name` / `fileId` / `parentId`
- `symbols[].kind`（已为具体类型，仍需 RC adapter 映射为 NodeLabel）
- `edges.*[].sourceId` / `targetId`（端点已归一化）
- `edges.*[].confidence` / `reason`（源数据存在时透传；Cangjie 目前不强制要求）
- `stats.*`（所有统计字段）
- `diagnostics[]`（结构兼容）
- `schemaVersion` / `generatedAt` / `language` / `root`

### 8.3 需要 adapter 的字段

- `edges.*[].kind`（需映射为 RelationshipType）
- Node/Edge ID（需格式转换）
- `packages[]` 中 target 节点是否转为 metadata-only（需 RC adapter 决策）

### 8.4 审计完整性

已读 GitNexus-RC 文件：16 个核心文件（含 bridge adapter 4 个），覆盖三条消费链（old adapter → shared types → web UI → LLM tools；bridge adapter → shared types → web UI → LLM tools）。未发现未知消费点。

### 8.5 二次审计确认（2026-05-09，v1.3.0）

本版本对 GitNexus-RC 消费侧进行第二轮只读审计，额外读取以下文件：

- `gitnexus-web/src/core/llm/tools.ts`（7 个 GraphRAG Tools：search/cypher/grep/read/overview/explore/impact）
- `gitnexus-web/src/lib/graph-adapter.ts`（knowledgeGraphToGraphology: Sigma.js 适配）
- `gitnexus-web/src/lib/constants.ts`（NODE_COLORS/NODE_SIZES/EdgeType/EDGE_INFO）
- `gitnexus-shared/src/graph/types.ts`（GraphNode/GraphRelationship/NodeLabel/RelationshipType）
- `gitnexus-shared/src/lbug/schema-constants.ts`（NODE_TABLES/REL_TYPES）
- `gitnexus/src/core/ingestion/rust-core-bridge-adapter/types.ts`（Bridge 类型/SYMBOL_KINDS/EDGE_KINDS/METADATA_ONLY）

**确认事项：**

1. **GraphRelationship 必需字段**：`id`（adapter 生成）、`sourceId`（bridge 已归一化）、`targetId`（bridge 已归一化）、`type`（adapter 映射）、`confidence`（bridge 透传/默认值）、`reason`（bridge 透传/默认值）。Bridge JSON 的 edge 字段经 adapter 映射后可完整满足。
2. **GraphRelationship 可选字段**：`step`（execution flow）、`evidence`（RFC #909 作用域解析）。Bridge 不提供，不影响现有功能。
3. **NodeLabel 覆盖率**：RC 定义 48 种 NodeLabel。Bridge symbol kind 覆盖其中 20 种（通过 adapter 的 SYMBOL_KIND_TO_LABEL 映射）。未覆盖的均为 RC 抽象节点（Community/Process/Section）或非适用语言（Template/Route/Tool）。
4. **RelationshipType 覆盖率**：RC 定义 24 种 RelationshipType。Bridge edge kind 覆盖其中 8 种常用类型（CALLS/DEFINES/USES/ACCESSES/IMPORTS/MODIFIES/ANNOTATES/MEMBER_OF）+ CONTAINS（通过多对一映射）。未覆盖的（EXTENDS/IMPLEMENTS/INHERITS 等）均为 Rust stop-line 后能力（无 type inference）。
5. **LLM Tools 兼容性**：impact 的默认关系类型（CALLS/IMPORTS/EXTENDS/IMPLEMENTS）中，CALLS + IMPORTS 已在 bridge 覆盖；EXTENDS/IMPLEMENTS 不在 Rust stop-line 内。explore/overview 查询依赖的 node.label（NodeLabel）/rel.type（RelationshipType）经 adapter 映射后匹配。
6. **Web graph-adapter 兼容性**：EDGE_STYLES 含 11 种样式（不含 USES，但其常量 EdgeType/EDGE_INFO 含 USES）。经 adapter 映射后的关系类型均可正确渲染。
7. **Rust-core bridge_roundtrip 增强**：
   - `assert_bridge_structure` 新增 `generatedAt` 字段检查（ISO 8601 时间戳，RC adapter 消费）
   - Cangjie symbol kind 白名单新增 `CallableSource`（对齐 RC adapter CANGJIE_SYMBOL_KINDS）
   - `known_adapter_mappings` 表使用 `Option<&str>` 区分直接映射与 metadata-only 跳过（CONTAINS_WORKSPACE/HAS_TARGET/RESOLVES_TO）

**结论：** 二次审计确认 bridge JSON 与 GitNexus-RC 消费侧的兼容性边界未变。所有结构性差异仍需 adapter 层转换（已在 GitNexus-RC `rust-core-bridge-adapter/` 实现），bridge JSON 本身结构完整且可被 adapter 正确消费。

### 8.6 三次审计确认（2026-05-09，v1.4.0）

本版本对 GitNexus-RC 消费侧进行第三轮只读审计，完整阅读 bridge adapter 全部 4 个文件（types.ts / validate.ts / map-to-gitnexus.ts / index.ts），以及 LLM tools / graph-adapter / constants / shared types 的详细实现。

**新发现：**

1. **Bridge adapter 已完整实现：** `rust-core-bridge-adapter/` 是专门消费 `--format gitnexus-rc` bridge JSON 的完整 adapter，包含：
   - **types.ts**：13 Rust + 9 Cangjie symbol kind 白名单、21 种 edge kind 白名单、10 种 edge 分组名
   - **validate.ts**：10 条验证规则（V1 顶层字段、V2 repository 完整性、V3 language 枚举、V4 symbol kind 按语言白名单、V5 duplicate ID、V6 dangling 端点、V7 edge kind 白名单、V8 旧字段检测、V9 stats 一致性、V10 最小合理性）
   - **map-to-gitnexus.ts**：6-phase 映射 pipeline（repo→pkg→file→sym→flat edges→diagnostics），24 symbol kind→NodeLabel 映射，24 edge kind→RelationshipType 映射（4 个为 null=metadata-only），confidence/reason 按语言策略（Rust 透传，Cangjie: structural=1.0, semantic=0.85），ID 重建（bridge 前缀 → RC `{path}:{Label}:{name}` 格式）
   - **index.ts**：`loadRustCoreBridgeGraph()` 完整 6 步 pipeline（read→parse→check→validate→map→warnings）
   - 与旧 `rust-core-graph-adapter/` 完全独立，两条路径互不干扰

2. **`evidence` 字段不适用：** `GraphRelationship` 有可选 `evidence` 字段（RFC #909 Ring 2 PKG #925 scope-based resolution 用），bridge JSON 不提供此字段。该字段为可选且向后兼容，不提供不影响现有功能。

3. **Web graph-adapter USES 边样式缺失：** `graph-adapter.ts` 的 `EDGE_STYLES` 定义 11 种样式（CONTAINS/DEFINES/IMPORTS/CALLS/EXTENDS/IMPLEMENTS/ANNOTATES/MODIFIES/ACCESSES/MEMBER_OF/STEP_IN_PROCESS），不含 USES。Cangjie bridge 的 `uses` 边经 adapter 映射为 `USES` RelationshipType 后可正确存入 KnowledgeGraph，但在 Sigma.js 可视化中不会渲染（因为样式未定义，不作为错误处理）。`constants.ts` 的 `ALL_EDGE_TYPES` 和 `EDGE_INFO` 均含 USES，为未来渲染留好接口。

4. **confidence/reason 按语言差异处理已对齐：**
   - Rust bridge 边：confidence/reason 从原始 edge properties 提升到顶层，bridge adapter 直接透传
   - Cangjie bridge 边：confidence/reason 为 null，bridge adapter 按 edge 类型给默认值（structural: 1.0, semantic: 0.85），reason 生成 `rust-core-cangjie-{kind}` 格式
   - 此差异已在 bridge adapter 层妥善处理，不影响消费侧

5. **CallableSource 合成节点已预留映射：** Cangjie 合成节点 `kind: "CallableSource"` 在 bridge adapter 中映射为 `CodeElement`（fallback 通用类型），不在 Web UI filterable labels 中默认显示，但可通过 search/cypher/impact 工具查询到。

6. **Bridge 确定性测试修复：** `bridge_roundtrip` 的 `assert_deterministic_output` 原来未排除 `generatedAt`（每次运行的时间戳不同），导致 Cangjie feature 下 `bridge_rust_deterministic` 间歇性失败。已修复为比较前先 strip `generatedAt`，与 `verify-bridge.sh` 策略一致。

**结论：** 三次审计确认 bridge JSON 与 GitNexus-RC 消费侧的兼容性完整。`rust-core-bridge-adapter/` 已实现对 bridge JSON 的完整 6 步消费 pipeline（read→parse→validate→map→KnowledgeGraph），验证规则覆盖 10 个维度，映射覆盖 24 种 symbol kind 和 24 种 edge kind。Rust-core bridge JSON 满足"随时可接入 GitNexus-RC adapter"的状态。

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.4.0 | 三次审计确认：完整阅读 bridge adapter 全部 4 个文件（types/validate/map-to-gitnexus/index）；修正 §1.2 消费路径（标注两条 adapter 路径）；§1.1 表格新增 bridge adapter 4 文件；新增 §8.6：记录 bridge adapter 完整架构（10 验证规则、6-phase 映射、confidence/reason 策略）、evidence 字段不适用、USES 边样式缺失、CallableSource 映射、deterministic 测试修复 |
| 2026-05-09 | 1.3.0 | 二次审计确认：重读 6 个消费侧文件（LLM tools / graph-adapter / constants / shared types / schema constants / bridge adapter types）；新增 §8.5 确认 GraphRelationship 必需/可选字段对齐、NodeLabel/RelationshipType 覆盖率、LLM Tools 兼容性；bridge_roundtrip 增强（generatedAt 检查、CallableSource 白名单、metadata-only 映射表精度） |
| 2026-05-09 | 1.2.0 | 文档一致性修复：§3.3 更新 confidence/reason 为"已补齐"；§5.4 改为"已修复"并标注 Cangjie 残留差异；§6.1 标记高/中优先级项为已完成 |
| 2026-05-09 | 1.1.0 | Follow-up 对齐：记录 `symbols[].kind` 具体化、edge `confidence` / `reason` 顶层透传、edge kind compatibility tests 已完成；剩余问题明确收束到 GitNexus-RC adapter 层 |
| 2026-05-09 | 1.0.0 | 初始 dry-run：GitNexus-RC 消费侧完整审计、字段匹配表、node/edge kind 映射表、已知风险分类、下一步建议 |
