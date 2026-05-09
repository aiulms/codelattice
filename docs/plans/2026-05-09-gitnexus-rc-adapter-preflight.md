# GitNexus-RC Adapter Preflight — Bridge JSON 接入

> **日期：** 2026-05-09
> **版本：** v1.1.0
> **状态：** Preflight → **已落地（bridge adapter 已 landed in GitNexus-RC）**
> **Stop-line：** 本文档记录 bridge adapter 接入预研与落地后的状态。**当前不执行任何 GitNexus-RC 修改。**

---

## 目的

为 GitNexus-RC 新增 `loadRustCoreBridgeGraph()` adapter 路径做预研，说明：
1. Bridge adapter 已落地 GitNexus-RC（commit `26a21b5e`，closure `75107091`）
2. Rust-core bridge JSON → GitNexus-RC KnowledgeGraph 的转换边界
3. 已知风险与缓解策略
4. 落地后的验证状态

**授权状态：** ✅ Bridge adapter 已在 GitNexus-RC 落地。Rust-core 侧已就绪。

---

## 一、当前状态

### 1.1 Rust-core Bridge JSON（已有）

Rust-core CLI 通过 `--format gitnexus-rc` 产出 bridge JSON：
- 顶层：`repository` / `packages` / `sourceFiles` / `symbols` / `edges`（分组）/ `diagnostics` / `stats`
- 端点字段已归一化为 `sourceId`/`targetId`
- symbol `kind` 已填入具体类型（非通用 "symbol"）
- edge `confidence`/`reason` 已提升到顶层（Rust 有值，Cangjie 为 null）
- 详情见 `docs/architecture/consumer-contract.md`

### 1.2 GitNexus-RC Bridge Adapter（✅ 已落地）

GitNexus-RC 已有 **两条** adapter 路径：

1. **`rust-core-graph-adapter/`**（旧）：消费 Rust-core 原始 GraphOutput（flat `nodes[]` + `edges[]`，`source`/`target` 端点），schema v0.2.0

2. **`rust-core-bridge-adapter/`**（新，✅ 已落地）：消费 `--format gitnexus-rc` bridge JSON，4 个文件：
   - `types.ts`（185 行）：`BridgeGraphOutput` 接口 + 13 Rust + 9 Cangjie symbol kind 白名单 + 21 edge kind 白名单 + 10 edge 分组名 + 4 metadata-only kind
   - `validate.ts`（199 行）：10 条验证规则（顶层字段、language、symbol kind 按语言白名单、duplicate ID、dangling 端点、edge kind 白名单、旧字段检测、stats 一致性、最小合理性）
   - `map-to-gitnexus.ts`（413 行）：6-phase 映射 pipeline，24 kind→NodeLabel 映射，24 kind→RelationshipType 映射（4 个 null=metadata-only），confidence/reason 按语言策略（Rust 透传，Cangjie: structural=1.0, semantic=0.85），ID 重建
   - `index.ts`（124 行）：`loadRustCoreBridgeGraph()` 入口 — read→parse→check→validate→map→warnings

两条路径独立，互不干扰。Bridge adapter 已落地 GitNexus-RC commit `26a21b5e`（closure `75107091`）。

### 1.3 Rust-core Consumer Dry-run（✅ 已完成）

三次审计（v1.4.0）：完整审计 GitNexus-RC 消费侧 16 个核心文件，确认 bridge JSON 与 KnowledgeGraph 消费链兼容。

---

## 二、Write Set（✅ 已实现）

以下为 GitNexus-RC repo 中 bridge adapter 的**实际落地的文件清单**：

### 2.1 新建文件（4 个，~921 行）

| 文件 | 实际行数 | 作用 | 状态 |
|------|---------|------|------|
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/types.ts` | 185 | Bridge JSON 类型定义 + 21 edge kind 白名单 + 10 edge 分组 | ✅ 已落地 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/validate.ts` | 199 | 10 条验证规则（V1-V10）：顶层字段、symbol kind 按语言白名单、edge kind 白名单、端点完整性、stats 一致性等 | ✅ 已落地 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/map-to-gitnexus.ts` | 413 | 6-phase 映射 pipeline：24 kind→NodeLabel + 24 kind→RelationshipType 映射、ID 重建、confidence/reason 按语言策略 | ✅ 已落地 |
| `gitnexus/src/core/ingestion/rust-core-bridge-adapter/index.ts` | 124 | `loadRustCoreBridgeGraph()` 入口：read→parse→check→validate→map→warnings | ✅ 已落地 |

### 2.2 总变更量

- **新建：** 4 个文件，~921 行（超出预研 ~320 行，因为实现了完整的 10 规则验证 + 6-phase 映射 + 按语言 confidence/reason 策略）
- **共享类型修改：** 无需（`EnumVariant`/`Constructor` 已在 RC `NodeLabel` 中存在；`USES`/`MODIFIES` 已在 RC `RelationshipType` 中存在）
- **总变更量：** ~921 行（仅新建，无修改已有文件）

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

## 六、授权 Gate（✅ 已完成）

**Bridge adapter 已落地 GitNexus-RC，无需再次授权：**

- ✅ GitNexus-RC `rust-core-bridge-adapter/` 已新建（4 个文件，~921 行）
- ✅ `EnumVariant`/`Constructor` 已在 RC `NodeLabel` 中存在（`gitnexus-shared/src/graph/types.ts`）
- ✅ `USES`/`MODIFIES`/`IMPORTS` 已在 RC `RelationshipType` 中存在（`gitnexus-shared/src/graph/types.ts`）
- ✅ Web UI `constants.ts` 已有 `EnumVariant`/`Constructor` 颜色和尺寸定义
- ✅ bridge adapter 可从 Rust-core bridge JSON fixture 正确消费

**Rust-core 侧已就绪且可独立验证的内容：**
- ✅ Bridge JSON 输出格式（`--format gitnexus-rc`）
- ✅ 端点完整性（bridge_roundtrip 26 tests，0 dangling）+ deterministic（排除 generatedAt）
- ✅ Consumer contract 文档（Tier 1/2/3 三级分类，v1.2.0）
- ✅ Adapter readiness tests（symbol kind whitelist, edge kind compatibility, packageId consistency）
- ✅ Bridge 验证脚本（`scripts/verify-bridge.sh`）
- ✅ Cross-repo consumer dry-run（v1.4.0，三次审计确认兼容性完整）

---

## 七、当前状态与后续

**已落地（Rust-core 侧）：**
1. ✅ Bridge JSON 输出稳定、确定性、0 dangling
2. ✅ 26 bridge_roundtrip tests（13 Rust + 13 Cangjie）
3. ✅ Bridge 兼容性三次审计（consumer-dry-run.md v1.4.0）

**已落地（GitNexus-RC 侧，commit `26a21b5e`）：**
1. ✅ `rust-core-bridge-adapter/` 完整实现（4 文件，~921 行）
2. ✅ 10 验证规则 + 6-phase 映射 pipeline
3. ✅ 24 kind→NodeLabel + 24 kind→RelationshipType 映射表

**待做（不阻塞 Rust-core alpha trial）：**
- GitNexus-RC Tool propagation：bridge adapter 集成到 GitNexus-RC Tool CLI
- 端到端验证：Rust-core bridge JSON → RC adapter → Web UI / LLM Tools 完整链路
- USES edge Sigma.js 渲染样式补充（`graph-adapter.ts` EDGE_STYLES 目前不含 USES）

**回滚安全性：** bridge adapter 为独立路径，不影响现有 `rust-core-graph-adapter/`
