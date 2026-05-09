# GitNexus-RC Adapter Preflight — Bridge JSON 接入

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Preflight（docs-only）
> **Stop-line：** 本文档为未来接入预研，**当前不执行任何 GitNexus-RC 修改**。

---

## 目的

为 GitNexus-RC 新增 `loadRustCoreBridgeGraph()` adapter 路径做最小预研，说明：
1. 未来如果授权修改 GitNexus-RC，最小 write set 是哪些文件
2. Rust-core bridge JSON → GitNexus-RC KnowledgeGraph 的转换边界
3. 已知风险与缓解策略
4. 最小验收标准

**授权状态：** ⚠️ 需要用户显式授权。当前 loop 不执行任何 GitNexus-RC 修改。

---

## 一、当前状态

### 1.1 Rust-core Bridge JSON（已有）

Rust-core CLI 通过 `--format gitnexus-rc` 产出 bridge JSON：
- 顶层：`repository` / `packages` / `sourceFiles` / `symbols` / `edges`（分组）/ `diagnostics` / `stats`
- 端点字段已归一化为 `sourceId`/`targetId`
- symbol `kind` 已填入具体类型（非通用 "symbol"）
- edge `confidence`/`reason` 已提升到顶层（Rust 有值，Cangjie 为 null）
- 详情见 `docs/architecture/consumer-contract.md`

### 1.2 GitNexus-RC 现有 adapter（已有）

GitNexus-RC 已有 `rust-core-graph-adapter/`，消费 Rust-core **原始** GraphOutput 格式：
- `validate.ts`：schema version、node/edge label 白名单、duplicate check
- `map-to-gitnexus.ts`：label→NodeLabel、edge type→RelationshipType、ID 重建、confidence/reason 注入
- `index.ts`：`loadRustCoreGraph()` 入口

该 adapter **不消费 bridge JSON**。Bridge JSON 是新格式，需要新增独立 adapter 路径。

---

## 二、最小 Write Set

以下为 GitNexus-RC repo 中**建议修改的文件清单**，基于只读审计（dry-run 报告已覆盖 11 个文件）：

### 2.1 必须新建

| 文件 | 预估行数 | 作用 |
|------|---------|------|
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/types.ts` | ~50 | Bridge JSON 类型定义（BridgeGraphOutput 接口 + bridge edge kind/group 枚举） |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/validate.ts` | ~80 | Bridge JSON 合约验证（顶层字段、symbol kind 白名单、edge kind 白名单、端点完整性、stats 一致性） |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/map-to-gitnexus.ts` | ~150 | Bridge→KnowledgeGraph 映射：ID 重建、kind→NodeLabel、edge kind→RelationshipType、grouped→flat edges |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/index.ts` | ~40 | 入口：`loadRustCoreBridgeGraph()` — JSON → validate → map → KnowledgeGraph |

### 2.2 可能需要修改

| 文件 | 变更 | 风险 |
|------|------|------|
| `gitnexus-shared/src/graph/types.ts` | 新增 NodeLabel 值：`EnumVariant`（Rust）、`Init`（Cangjie）、`CallableSource`（Cangjie 合成节点，可选） | 低（enum 扩展向后兼容） |
| `gitnexus-shared/src/graph/types.ts` | 新增 RelationshipType 值：`DESIGNATION`（Rust 专属） | 低（enum 扩展向后兼容） |
| `gitnexus-shared/src/lbug/schema-constants.ts` | 新增 node/rel 表条目 | 低（追加列表） |
| `gitnexus-web/src/lib/constants.ts` | 新增 NodeLabel 颜色/大小、EdgeType 样式 | 低（追加映射表，不消费时无效果） |
| `gitnexus-web/src/lib/graph-adapter.ts` | 无需修改（Sigma.js adapter 仅消费 label/type 字符串，不依赖枚举） | 无 |

### 2.3 不需要修改

| 文件 | 原因 |
|------|------|
| `gitnexus/src/core/graph/types.ts` | KnowledgeGraph 接口不变 |
| `gitnexus/src/core/graph/graph.ts` | KnowledgeGraph 实现不变 |
| `gitnexus-web/src/core/llm/tools.ts` | LLM tools 动态发现 node/rel 类型，无需硬编码 |
| `gitnexus/src/core/ingestion/rust-core-graph-adapter/*` | 现有 adapter 路径保留不动，bridge adapter 为独立路径 |

### 2.4 文件变更总数

- **新建：** 4 个文件，~320 行
- **修改：** 2-3 个文件，~30 行
- **总变更量：** ~350 行

---

## 三、Bridge JSON → KnowledgeGraph 转换边界

### 3.1 ID 重建

Bridge JSON 使用 `repo:`/`pkg:`/`target:`/`file:`/`sym:` 前缀 ID，RC KnowledgeGraph 使用 `{filePath}:{NodeLabel}:{name}` 格式。

| Bridge ID | RC KnowledgeGraph ID（adapter 重建后） |
|-----------|--------------------------------------|
| `repo:project-name` | 保持不变（repository 为单例） |
| `pkg:Cargo.toml` | `Cargo.toml:Package:my-crate` |
| `target:my-crate::lib` | metadata-only（不映射为 Node） |
| `file:src/lib.rs` | `src/lib.rs:File:lib.rs` |
| `sym:portable-smoke::crate::Calculator` | `src/lib.rs:Struct:Calculator` |
| `sym:cangjie:portable-smoke:Init:MyClass.init#1` | `src/main.cj:Constructor:MyClass.init#1` |

**风险：** ID 重建依赖 `symbols[].kind` 映射为 `NodeLabel` 的正确性。如果 kind 不在已知映射表中，adapter 需有 fallback 策略（如 `CodeElement`）。

### 3.2 Node Kind 映射

Bridge symbol `kind` → RC `NodeLabel` 映射表：

| Bridge kind | RC NodeLabel | 备注 |
|------------|-------------|------|
| `function` | `Function` | |
| `method` | `Method` | |
| `associated-function` | `Method` | Rust 特有 |
| `struct` | `Struct` | |
| `enum` | `Enum` | |
| `trait` | `Trait` | Rust 特有 |
| `impl-block` | `Impl` | Rust 特有 |
| `const` | `Const` | |
| `static` | `Static` | Rust 特有 |
| `macro-definition` | `Macro` | Rust 特有 |
| `type-alias` | `TypeAlias` | |
| `module` | `Module` | |
| `enum-variant` | `EnumVariant` | **需新增 NodeLabel** |
| `Class` | `Class` | Cangjie 特有 |
| `Interface` | `Interface` | Cangjie 特有 |
| `Init` | `Constructor` | Cangjie 特有，**或需新增 Init** |
| `TypeAlias` | `TypeAlias` | Cangjie |
| `Macro` | `Macro` | Cangjie |
| `Function` | `Function` | Cangjie |
| `Struct` | `Struct` | Cangjie |
| `Enum` | `Enum` | Cangjie |

**需要新增的 NodeLabel（2-3 个）：**
- `EnumVariant` — Rust `enum-variant`
- `CallableSource` — Cangjie 合成节点（如果 bridge 未来输出）— 可选
- `Init` 可映射到现有 `Constructor` 或新增独立 label（需决策）

### 3.3 Edge Kind 映射

Bridge edge `kind` → RC `RelationshipType` 映射表：

| Bridge edge kind | RC RelationshipType | 备注 |
|-----------------|---------------------|------|
| `CALLS` | `CALLS` | 直接对应 |
| `DEFINES` / `defines` | `DEFINES` | |
| `uses` | `USES` | Cangjie call reference |
| `accesses` / `ACCESSES` | `ACCESSES` | |
| `imports` | `IMPORTS` | Cangjie |
| `modifies` | `MODIFIES` | Cangjie，在 other 分组 |
| `DESIGNATION` | `ANNOTATES` | Rust，映射为 ANNOTATES |
| `annotates` / `ANNOTATES` | `ANNOTATES` | |
| `CONTAINS_PACKAGE` / `containsPackage` | `CONTAINS` | |
| `CONTAINS_WORKSPACE` / `containsWorkspace` | metadata-only | RC 不映射 |
| `HAS_TARGET` / `hasTarget` | metadata-only | RC 不映射 |
| `OWNS_SOURCE` / `ownsSource` | `CONTAINS` | 映射到 CONTAINS |
| `HAS_PARENT` / `hasParent` | `MEMBER_OF` | |
| `RESOLVES_TO` / `resolvesTo` | metadata-only | 可选 MEMBER_OF |

### 3.4 Edge 分组 → 扁平化

Bridge JSON 的 `edges` 是分组结构（`calls`/`defines`/`uses`/...），RC KnowledgeGraph 的 `relationships` 是扁平数组。Adapter 需将所有分组合并为一个扁平数组。

### 3.5 Confidence/Reason 处理

- **Rust bridge：** `confidence` 和 `reason` 已有值，直接透传
- **Cangjie bridge：** `confidence` 和 `reason` 均为 `null`，adapter 需决策默认值或跳过
  - 建议：structural edges（DEFINES/CONTAINS/MEMBER_OF）默认 confidence=1.0；semantic edges（USES/IMPORTS/MODIFIES）默认 confidence=0.85 或由 consumer 决定是否必须

### 3.6 Diagnostic 处理

RC adapter 当前不将 diagnostic 映射为 KnowledgeGraph node（仅统计）。Bridge JSON 保留了 diagnostics 数组，未来如需消费需扩展 adapter。

### 3.7 Packages 含 Target 节点

Rust bridge 将 `target` 节点（lib/bin）也放入 `packages` 数组（因为 Rust package+target 共用 BridgePackage 结构）。RC adapter 需按 `language` 过滤或按 ID 前缀 `target:` 识别并跳过。

---

## 四、风险与缓解

| 风险 | 等级 | 缓解 |
|------|------|------|
| Node ID 格式差异 | **中** | Bridge 与 RC 使用不同 ID 策略，adapter 重建 ID。已有完整映射表（§3.1），bridge roundtrip 测试覆盖端点完整性 |
| NodeLabel 缺失 `EnumVariant`/`Init` | **中** | 需在 `gitnexus-shared` 扩展 NodeLabel 枚举。后端 enum 扩展向后兼容，前端需新增颜色/样式条目 |
| Edge type 不完全对齐 | **中** | Bridge 保留原始 type 名称，16 个已映射到 RC RelationshipType（§3.3），其余进入 metadata-only |
| Cangjie confidence/reason 为 null | **低** | Adapter 需处理 null 情况（建议默认值方案），不影响 Rust bridge |
| packages 语义差异（含 target） | **低** | 按 `language` 或 ID 前缀过滤即可 |
| Rust/Cangjie 双语言 schema 差异 | **中** | Adapter 按 `language` 字段分支处理，两种语言不混用 |
| 前端不渲染 newNodeLabel/EdgeType | **低** | 前端 Sigma.js adapter 动态渲染 label 字符串，不消费时无效果；仅需新增颜色/样式 |

---

## 五、最小验收标准

GitNexus-RC adapter 接入的最小验收清单：

### 5.1 单元级

1. **validate.ts** 能解析 Rust bridge JSON fixture，无 rejection
2. **map-to-gitnexus.ts** 能正确映射：
   - 所有 symbol kind → NodeLabel（无遗漏、无错误映射）
   - 所有 edge kind → RelationshipType（无遗漏、无错误映射）
   - ID 重建后的节点 ID 格式正确且集合内唯一
3. **index.ts** `loadRustCoreBridgeGraph()` 调用链条完整，返回合法 `KnowledgeGraph`

### 5.2 集成级

4. **Rust** portable-smoke fixture bridge JSON 能完整通过 adapter pipeline
5. **Cangjie** portable-smoke fixture bridge JSON 能完整通过 adapter pipeline
6. Adapter 产出的 KnowledgeGraph 能被 Sigma.js Web UI 渲染（节点/边可见）
7. Adapter 产出的 KnowledgeGraph 能被 LLM tools（search/impact/explore）消费

### 5.3 合约级

8. Bridge JSON 的 `confidence`/`reason` 值不丢失（Rust bridge 语义边）
9. Bridge JSON 的端点完整性通过 validate（无 dangling source/target）
10. Rust 和 Cangjie bridge 的 KnowledgeGraph 节点/边类型正确区分
11. `stats` 字段与 KnowledgeGraph 实际节点/边数一致

---

## 六、授权 Gate

**以下操作需要用户显式授权，当前不执行：**

1. 新建或修改 GitNexus-RC `gitnexus/src/core/ingestion/` 下任何文件
2. 修改 GitNexus-RC `gitnexus-shared/src/graph/types.ts` NodeLabel/RelationshipType 枚举
3. 修改 GitNexus-RC `gitnexus-web/` 前端常量/适配器
4. 运行 GitNexus-RC adapter 或 pipeline 测试

**Rust-core 侧已就绪且可独立验证的内容：**
- ✅ Bridge JSON 输出格式（`--format gitnexus-rc`）
- ✅ 端点完整性（bridge_roundtrip 26 tests，0 dangling）
- ✅ Consumer contract 文档（Tier 1/2/3 三级分类）
- ✅ Adapter readiness tests（symbol kind whitelist, edge kind compatibility, packageId consistency）
- ✅ Bridge 验证脚本（`scripts/verify-bridge.sh`）

---

## 七、下一步

1. **用户授权后：** 在 GitNexus-RC 新建 `rust-core-bridge-adapter/` 目录，参考本文档 §二 write set
2. **实现顺序建议：** types.ts → validate.ts → map-to-gitnexus.ts → index.ts → 测试 → 前端 NodeLabel 扩展
3. **验收：** 按 §五 验收清单逐项通过
4. **回滚：** 新建独立 adapter 路径不影响现有 `rust-core-graph-adapter/`，回滚仅需删除新目录 + revert types.ts 枚举扩展
