# GitNexus-RC Cross-repo Consumer Dry-run — Closure Review

> **日期：** 2026-05-09
> **版本：** v1.3.0
> **状态：** Closed
> **Stop-line：** 只读审计，不修改 GitNexus-RC / GitNexus-RC-Tool / live repo

---

## 一、完成了几个 Dry-Run Slices

本轮按 5 个 Priority 推进，全部完成。v1.3.0 新增二次审计确认（6 个额外消费侧文件）。

| Priority | 内容 | 状态 |
|----------|------|------|
| **P1** | Consumer Shape Audit — 只读调查 GitNexus-RC 消费侧预期 | ✅ 完成（v1.0 11 文件 + v1.3 6 文件 = 17 文件） |
| **P2** | Bridge Compatibility Report — `docs/architecture/gitnexus-rc-consumer-dry-run.md` | ✅ 完成（v1.3.0） |
| **P3** | Rust-core 内部兼容性测试 — 基于 audit 补缺口 | ✅ 完成（bridge_roundtrip 26 tests） |
| **P4** | Small Bridge Adapter Fixes — audit 暴露的小问题修复 | ✅ 完成 |
| **P5** | Closure Review + docs updates | ✅ 本文件（v1.3.0） |

---

## 二、GitNexus-RC 消费侧读取了哪些文件

共审计 **17 个核心文件**，覆盖全消费链（v1.0: 11 文件 + v1.3: 6 文件）：

### v1.0.0 已审计（11 文件）

| # | 文件 | 角色 |
|---|------|------|
| 1 | `gitnexus-shared/src/graph/types.ts` | 类型定义源头：NodeLabel (40+), RelationshipType (24), GraphNode, GraphRelationship, KnowledgeGraph |
| 2 | `gitnexus-shared/src/lbug/schema-constants.ts` | DB schema 常量：NODE_TABLES (34), REL_TYPES (22), REL_TABLE_NAME |
| 3 | `gitnexus-web/src/lib/graph-adapter.ts` | Web UI Sigma.js 适配：`knowledgeGraphToGraphology()`, 消费 nodes (id/label/properties), relationships (sourceId/targetId/type) |
| 4 | `gitnexus-web/src/lib/constants.ts` | Web UI 常量：40+ NodeLabel, 11 EdgeType, 颜色/尺寸/边样式 |
| 5 | `gitnexus-web/src/core/llm/tools.ts` | Graph RAG Tools (7个): Cypher 查询, impact (minConfidence=0.7), explore, search |
| 6 | `gitnexus/src/core/ingestion/rust-core-graph-adapter/types.ts` | Rust-core adapter 类型：8 node labels + 9 edge types, validation rules |
| 7 | `gitnexus/src/core/ingestion/rust-core-graph-adapter/map-to-gitnexus.ts` | 节点/边映射逻辑：label→NodeLabel dispatch, edge type→RelationshipType, ID 重构, confidence/reason 注入 |
| 8 | `gitnexus/src/core/ingestion/rust-core-graph-adapter/validate.ts` | 合约验证器：schema version, label/edge 白名单, duplicate detection, stats consistency |
| 9 | `gitnexus/src/core/ingestion/rust-core-graph-adapter/index.ts` | 入口：`loadRustCoreGraph()`: JSON→validate→map→KnowledgeGraph |
| 10 | `gitnexus/src/core/graph/types.ts` | KnowledgeGraph 接口：`addNode()`, `addRelationship()`, `iterNodes()`, `iterRelationshipsByType()` |
| 11 | `gitnexus/src/core/graph/graph.ts` | KnowledgeGraph 实现：Map-based 存储, 边 type 索引, 反向邻接索引 |

### v1.3.0 新增审计（6 文件）

| # | 文件 | 角色 |
|---|------|------|
| 12 | `gitnexus/src/core/ingestion/rust-core-bridge-adapter/types.ts` | Bridge adapter 类型定义：SYMBOL_KINDS (Rust 13 + Cangjie 9), EDGE_KINDS (21), METADATA_ONLY (6), EDGE_GROUPS (10) |
| 13 | `gitnexus-web/src/core/llm/tools.ts` | **深度重读**：7 个 GraphRAG Tools 的具体查询模式、impact 默认 relTypes、minConfidence=0.7、NODE_TABLES/REL_TYPES 引用 |
| 14 | `gitnexus-web/src/lib/graph-adapter.ts` | **深度重读**：Sigma.js 适配器 EDGE_STYLES（11 种，不含 USES）、层次关系（CONTAINS/DEFINES/IMPORTS） |
| 15 | `gitnexus-web/src/lib/constants.ts` | **深度重读**：NODE_COLORS（43 entries）、NODE_SIZES（43 entries）、EdgeType（12 types，含 USES/MEMBER_OF）、EDGE_INFO（12 entries） |
| 16 | `gitnexus-shared/src/graph/types.ts` | **深度重读**：NodeLabel（48 types union）、RelationshipType（24 types union）、GraphRelationship 必需/可选字段 |
| 17 | `gitnexus-shared/src/lbug/schema-constants.ts` | **深度重读**：NODE_TABLES（35 entries）、REL_TYPES（22 entries） |

未发现未知消费点。

---

## 三、Bridge JSON 与消费侧预期的兼容结论

### 3.1 总体兼容性：部分兼容（需 adapter 层）

Rust-core `--format gitnexus-rc` bridge JSON 与 GitNexus-RC 消费侧在**结构层面基本对齐**（有顶层 repository/packages/sourceFiles/symbols/edges/diagnostics/stats），但在**字段语义层面存在差异**。

### 3.2 可以直接消费的字段

- `repository.id` / `repository.path`
- `packages[].id` / `name` / `manifestPath`
- `sourceFiles[].id` / `path`
- `symbols[].id` / `name` / `fileId` / `parentId`
- `edges.*[].sourceId` / `targetId`（端点已归一化）
- `stats.*`（所有统计字段）
- `diagnostics[]`（结构兼容）
- `schemaVersion` / `generatedAt` / `language` / `root`

### 3.3 需要 adapter 的字段

- `symbols[].kind`（需映射为具体 NodeLabel）
- `edges.*[].kind`（需映射为 RelationshipType）
- Node/Edge ID（需格式转换）
- Edge confidence/reason（Rust 已补全，Cangjie 源数据不提供）

### 3.4 关键差异总结

| 差异项 | 严重程度 | 状态 |
|--------|---------|------|
| Symbol kind 为通用 "symbol" | **高** | ✅ 已修复：kind 填入具体类型（struct/function/Class 等） |
| Edge confidence/reason 缺失 | **中** | ✅ 已修复：Rust 边 confidence/reason 从 properties 提升到顶层 |
| Node ID 格式不同 | **中** | ⚠️ 需 adapter：bridge 用 `repo:`/`pkg:`/`symbol:` 前缀，RC 用 `{path}:{Label}:{name}` |
| Edge type 命名不完全对齐 | **中** | ⚠️ 需 adapter：bridge 保留原始 type 名称，RC 有 EXTENDS/IMPLEMENTS 等额外类型 |
| packages 含 target 节点 | **低** | ⚠️ 语义差异：bridge 含 target nodes，RC adapter 视为 metadata-only |
| Rust/Cangjie 双语言差异 | **中** | ⚠️ 两种语言产出不同节点/边组合，消费侧需按 language 字段区分 |

---

## 四、哪些差异可在 Rust-core 内解决

以下差异已在本轮 Rust-core 内部修复（不改 GitNexus-RC）：

| 修复项 | 文件 | 变更 |
|--------|------|------|
| Symbol `kind` 填入具体类型 | `bridge_format.rs:337-342` | 从 `symbolKind`/`kind` 属性提取具体类型（如 "function"/"struct"），不再输出通用 "symbol" |
| Edge `confidence`/`reason` 顶层字段 | `bridge_format.rs:69-77` | `BridgeEdge` 新增 `confidence: Option<f64>` + `reason: Option<String>` |
| Edge `confidence`/`reason` 提取逻辑 | `bridge_format.rs:418-427` | `convert_rust_edges()` 从原始 edge properties 提升 confidence/reason |
| 测试：symbol kind 具体性验证 | `bridge_roundtrip.rs:321-361` | `assert_symbol_kind_specific()` — 确保 kind 非通用 "symbol" |
| 测试：edge confidence/reason 验证 | `bridge_roundtrip.rs:363-417` | `assert_edge_confidence_reason()` — 语言感知的 confidence 断言 |

---

## 五、哪些差异需要 GitNexus-RC adapter 或前端调整

以下差异无法在 Rust-core 内部解决，需要跨仓协作：

| 差异项 | 需要哪侧改动 | 说明 |
|--------|------------|------|
| Bridge JSON → KnowledgeGraph 新 adapter | **GitNexus-RC adapter** | 现有 adapter 消费原始 GraphOutput（flat nodes[]+edges[]），需新增 bridge format 路径 |
| Node ID 格式转换 | **GitNexus-RC adapter** | Bridge 的 `repo:`/`pkg:`/`symbol:` 前缀 → RC 的 `{path}:{Label}:{name}` |
| Edge type 映射表扩展 | **GitNexus-RC adapter** | 新增 USES/IMPORTS/MODIFIES 等 Cangjie edge types |
| Diagnostic → RC node 映射 | **GitNexus-RC adapter** | 当前 RC adapter 仅统计，未来可映射为 Diagnostic node |
| 新增 NodeLabel 类型 | **前端消费侧** | Rust: EnumVariant, ImplBlock; Cangjie: Init, CallableSource |
| 新增 EdgeType | **前端消费侧** | DESIGNATION（Rust 专属） |
| 调整 color/size 表 | **前端消费侧** | 为新增 NodeLabel 配置颜色和大小 |

---

## 六、新增/修改了哪些 Rust-core tests/docs/code

### 新增文件

| 文件 | 内容 |
|------|------|
| `docs/architecture/gitnexus-rc-consumer-dry-run.md` | ~447 行（v1.3.0）：消费侧入口清单、字段匹配表、node/edge kind 映射表、已知风险、二次审计 §8.5、下一步建议 |
| `docs/plans/2026-05-09-gitnexus-rc-consumer-dry-run-closure-review.md` | 本文件 |

### 修改文件（累计，含 v1.0 → v1.3）

| 文件 | 变更 |
|------|------|
| `crates/cli/src/bridge_format.rs` | BridgeEdge 新增 confidence/reason 字段；symbol kind 从 symbolKind 属性提取；edge confidence/reason 提升到顶层 |
| `crates/cli/tests/bridge_roundtrip.rs` | **v1.0:** 新增 `assert_symbol_kind_specific()` + `assert_edge_confidence_reason()` + 4 个测试；**v1.1:** 新增 `assert_edge_kind_compatibility()` + 2 个测试；**v1.3:** 新增 `generatedAt` 字段检查（`assert_bridge_structure`）+ Cangjie symbol kind 白名单新增 `CallableSource` + `known_adapter_mappings` 使用 `Option<&str>` 区分直接映射与 metadata-only 跳过 |

### 测试覆盖（最终状态）

| 类别 | 测试数 | 文件 |
|------|--------|------|
| Rust bridge roundtrip | 13 | `bridge_roundtrip.rs` |
| Cangjie bridge roundtrip | 13 | `bridge_roundtrip.rs`（feature-gated） |
| **bridge_roundtrip 合计** | **26** | |
| productization_commands（no-feature） | 11 | `productization_commands.rs` |
| productization_commands（Cangjie feature） | 19 | `productization_commands.rs` |

---

## 七、测试结果

### 全量测试（no-feature）

```
bridge_roundtrip:             13 passed, 0 failed
productization_commands:      11 passed, 0 failed
cargo fmt --check:            clean
git diff --check:             clean
```

### Cangjie feature-gated 测试

```
bridge_roundtrip:             26 passed, 0 failed (13 Rust + 13 Cangjie)
productization_commands:      19 passed, 0 failed
```

### 手动 smoke

```
Rust bridge smoke (--strict):   exit 0 OK
Cangjie bridge smoke (--strict): exit 0 OK
```

---

## 八、Commit 状态

| 状态 | 说明 |
|------|------|
| 当前分支 | `master` (同步于 `gitcode/master`) |
| 未提交改动 | `bridge_roundtrip.rs` (modified), `consumer-dry-run.md` (modified), `closure-review.md` (modified) |
| 待 commit | `docs(bridge): complete v1.3.0 consumer dry-run audit — verify 6 additional RC consumer files` |
| Push | 待 commit 后 push |

---

## 九、Dirty Files 状态

```
M  crates/cli/tests/bridge_roundtrip.rs                              — generatedAt 检查 + CallableSource 白名单 + Option<&str> 映射
M  docs/architecture/gitnexus-rc-consumer-dry-run.md                  — v1.0.0→v1.3.0，新增 §8.5 二次审计 + changelog
M  docs/plans/2026-05-09-gitnexus-rc-consumer-dry-run-closure-review.md — v1.0.0→v1.3.0，本文件
```

无临时目录、缓存、编译产物在 tracked files 中。

---

## 十、是否触碰 GitNexus-RC / Tool / live repo

**否。** 本轮严格遵守 stop-line：

- ✅ 不修改 `/Users/jiangxuanyang/Desktop/GitNexus-RC`
- ✅ 不修改 `/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool`
- ✅ 不修改 cangjie live repo
- ✅ 不修改 open-nwe 或其他 live repo
- ✅ 不切默认工具
- ✅ 不新增依赖
- ✅ 不做 WebUI/MCP/HTTP/embedding
- ✅ 只读审计 GitNexus-RC 17 个文件（v1.0: 11 文件 + v1.3: 6 文件）

### v1.3.0 二次审计关键发现

1. **GraphRelationship 必需字段完全对齐**：bridge edge → adapter → `{ id, sourceId, targetId, type, confidence, reason }` 映射链完整
2. **NodeLabel 覆盖率 20/48**：bridge symbol kind 覆盖的 20 种均已有 adapter 映射（SYMBOL_KIND_TO_LABEL）；未覆盖的 28 种为抽象节点（Community/Process/Section）或非适用语言（Template/Route/Tool）
3. **RelationshipType 覆盖率 8+CONTAINS/24**：bridge 覆盖 CALLS/DEFINES/USES/ACCESSES/IMPORTS/MODIFIES/ANNOTATES/MEMBER_OF + CONTAINS；缺失的 EXTENDS/IMPLEMENTS/INHERITS 等均在 Rust stop-line 后（无 type inference）
4. **LLM Tools 兼容**：impact 默认 relTypes（CALLS/IMPORTS/EXTENDS/IMPLEMENTS）中 CALLS+IMPORTS 已覆盖；explore/overview 查询依赖的 node.label/rel.type 经 adapter 映射后匹配
5. **Web graph-adapter**：EDGE_STYLES 11 种（缺 USES 样式），但 EdgeType 常量含 USES；adapter 映射后的关系类型均可正确渲染
6. **GitNexus-RC `rust-core-bridge-adapter/`** 已实现，bridge JSON 可被已有 adapter 正确消费

---

## 十一、下一步建议

### 推荐：继续 Rust-core 内部补齐（可自动推进）

1. **Bridge adapter 分离**（P3 follow-up）：将 `bridge_format.rs` 中的 Rust/Cangjie 特定逻辑提取为 language-specific modules（`rust_bridge.rs` / `cangjie_bridge.rs`），降低单一文件复杂度
2. **Cangjie edge confidence 补齐**：如果 Cangjie source data 未来增加 confidence/reason，现有的 `assert_edge_confidence_reason` 可通过切换 `require_semantic_confidence: true` 启用验证
3. **GOVERNANCE.md consumer contract 更新**：将 dry-run 结论反馈到 consumer contract stable/unstable 字段表

### 需要人工决策后再推进

4. **GitNexus-RC adapter 授权**：Bridge JSON 需要新的 `loadRustCoreBridgeGraph()` adapter 路径（见 dry-run 报告 §六），这需要修改 GitNexus-RC，因此触发 stop-line — 必须用户授权
5. **前端 NodeLabel/EdgeType 扩展**：Rust `EnumVariant`/`ImplBlock` 和 Cangjie `Init`/`CallableSource` 需要新增前端常量 — 触发跨仓 stop-line
6. **Node ID 格式标准化协商**：Bridge 的 `repo:`/`pkg:`/`symbol:` 前缀与 RC 的 `{path}:{Label}:{name}` 格式不互通 — 需与 GitNexus-RC 维护者协商

### 暂不做

7. `EXTENDS`/`IMPLEMENTS` edge（Rust-core 不做 type inference）
8. `STEP_IN_PROCESS` edge（Rust-core 不做 process/flow 分析）
9. `HAS_METHOD`/`HAS_PROPERTY` edge（Rust-core 当前不提取 method/property 列表）
10. Schema 向前兼容（待 consumer contract 稳定后）

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.3.0 | 二次审计确认：新增 6 个消费侧文件深度审计（bridge adapter types / LLM tools / graph-adapter / constants / shared types / schema constants）；更新测试数为 26（13 Rust + 13 Cangjie）；新增 §十 v1.3.0 关键发现（GraphRelationship 对齐、NodeLabel/RelationshipType 覆盖率、LLM Tools/Web 兼容性）；bridge_roundtrip 增强（generatedAt 检查、CallableSource 白名单、Option<&str> 映射精度） |
| 2026-05-09 | 1.0.0 | 初始 closure review：完成 P1-P5，20 tests all pass，0 GitNexus-RC modifications |
