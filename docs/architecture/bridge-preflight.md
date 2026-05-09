# Bridge Preflight — GitNexus Rust-core → GitNexus-RC 桥接准备

> **日期：** 2026-05-09
> **版本：** v1.2.0
> **状态：** Preflight → Implemented（Bridge adapter 已落地并拆分为 3 文件；GitNexus-RC 侧 `rust-core-bridge-adapter/` 已 landed）
> **Stop-line：** No production replacement. No GitNexus-RC modification. No GitNexus-RC-Tool modification.

---

## 目的

分析 Rust-core unified output 如何未来给 GitNexus-RC / frontend 消费，明确差异、可直接映射字段、需要 adapter 的字段。

**本文不涉及任何实现。** 仅作为桥接前的分析文档。

---

## 一、消费场景分析

### 场景 A：GitNexus-RC 前端消费

GitNexus-RC 前端期望的 graph JSON 格式（由 TS adapter 的 `GraphOutput` 定义）：

```
GraphOutput {
  repository: { id, name, root }
  packages: [{ id, name, manifestPath, ... }]
  targets: [{ id, name, packageId, ... }]
  sourceFiles: [{ id, path, packageId, ... }]
  symbols: [{ id, name, kind, fileId, ... }]
  edges: { calls: [...], uses: [...], defines: [...] }
  diagnostics: [...]
}
```

### 场景 B：AI Workflow 消费

AI workflow 期望通过 CLI JSON stdout 获取结构化 graph 数据，不依赖 MCP/HTTP。

---

## 二、Rust-core 现状 vs GitNexus-RC 格式比较

### 2.1 Rust GraphOutput (project-model)

```json
{
  "schemaVersion": "v0.3",
  "generatedAt": "...",
  "root": { "path": "...", "manifest": "..." },
  "nodes": [
    { "id": "repo:...", "label": "repository", "properties": {...} },
    { "id": "pkg:...", "label": "package", "properties": {...} },
    { "id": "sym:...", "label": "symbol", "properties": { "kind": "function", ... } }
  ],
  "edges": [
    { "type": "CALLS", "source": "...", "target": "...", "properties": {} }
  ],
  "diagnostics": [...],
  "stats": { "nodeCount": 1524, ... }
}
```

### 2.2 Cangjie CangjieGraphOutput (cangjie)

```json
{
  "nodes": [
    { "id": "repo:...", "kind": "repository", "label": "...", "properties": {} },
    { "id": "pkg:...", "kind": "package", "label": "...", "properties": {} }
  ],
  "edges": [
    { "kind": "uses", "sourceId": "...", "targetId": "..." }
  ]
}
```

### 2.3 关键差异矩阵

| 维度 | Rust (project-model) | Cangjie (cangjie) | GitNexus-RC (TS) |
|------|---------------------|-------------------|------------------|
| **顶层结构** | 扁平 node/edge 列表 | 扁平 node/edge 列表 | 分组：symbols/edges.calls/edges.uses |
| **Node kind** | label 字段（无显式 kind） | kind 枚举（显式序列化） | kind 枚举（显式序列化） |
| **Edge type** | `"type": "CALLS"` | `"kind": "uses"` | 按类型分组（calls/uses/defines） |
| **Edge 端点** | `source` / `target` | `sourceId` / `targetId` | `sourceId` / `targetId` |
| **Workspace** | ✅ CONTAINS_WORKSPACE | ❌ 无 workspace 概念 | ✅ workspace support |
| **Target** | ✅ HAS_TARGET (lib/bin) | ❌ 无 target 概念 | ✅ target support |
| **Module** | ✅ Module node + HAS_PARENT | ❌ 无 module 概念 | ✅ module support |
| **CALLS edge** | ✅ v0.2 | ❌ Uses/Accesses/Modifies 代替 | ✅ calls edges array |
| **Stats/stats** | ✅ stats 字段 | ❌ 无 stats | ✅ stats 字段 |

### 2.4 可直接映射的字段

以下字段在 Rust-core 和 GitNexus-RC 之间语义等价：

| Rust-core 字段 | GitNexus-RC 字段 | 映射方式 |
|---------------|-----------------|----------|
| `node.id` | `symbol.id` / `sourceFile.id` 等 | 直接映射 |
| `node.label` (symbol) | `symbol.name` | 直接映射 |
| `edges[].type = "CALLS"` | `edges.calls[]` | 按 type 分组 |
| `edges[].type = "DEFINES"` | `edges.defines[]` | 按 type 分组 |
| `diagnostics[]` | `diagnostics[]` | 直接映射 |
| `stats.symbolCount` | `stats.symbols` | 直接映射 |

### 2.5 需要 Adapter 的差异

| 差异 | Rust-core | GitNexus-RC | Adapter 策略 |
|------|-----------|-------------|-------------|
| **Node kind 编码** | label 字段（字符串） | 显式 kind 枚举 | Adapter 从 label 推断 kind |
| **Edge type 编码** | type 字段（大写字符串） | 按类型分组 | Adapter 按 type 分组 |
| **Edge 端点字段名** | `source` / `target`（Rust）/ `sourceId` / `targetId`（Cangjie） | `sourceId` / `targetId` | 统一为 `sourceId` / `targetId` |
| **Workspace/Target/Module** | Rust 有，Cangjie 无 | Rust 有 | Cangjie 跳过这些概念 |
| **Repository node** | Rust 和 Cangjie 都有 | 作为顶层字段 | Adapter 提取 repo 信息 |

---

## 三、Bridge Adapter 建议接口（仅设计参考）

如果未来实现 bridge adapter，建议接口：

```rust
/// 将 Rust-core 统一输出转换为 GitNexus-RC 兼容格式
fn to_gitnexus_rc_format(result: &LanguageAnalysisResult) -> GitNexusRCGraphOutput {
    // ...
}

/// 标准化 edge 端点字段名（统一为 sourceId / targetId）
fn normalize_edge_endpoints(edges: &[Value], language: &str) -> Vec<NormalizedEdge> {
    // Rust: source → sourceId, target → targetId
    // Cangjie: sourceId → sourceId, targetId → targetId（已是标准格式）
}
```

**注意：** 以上仅为接口参考，不作实现。

---

## 四、Stop-line 明确

以下操作 NEvER 在 Rust-core 中实现：

1. ❌ 不修改 GitNexus-RC `src/` 目录下任何代码
2. ❌ 不修改 GitNexus-RC `package.json` / `tsconfig.json`
3. ❌ 不修改 GitNexus-RC graph schema（`graph-schema-v0.md` 等）
4. ❌ 不修改 GitNexus-RC-Tool 任何文件
5. ❌ 不做 production replacement
6. ❌ 不将 bridge adapter 接入 GitNexus-RC

**可以做的：**
1. ✅ Rust-core 内部新增 export function / CLI flag（如 `--format gitnexus-rc`）
2. ✅ 新增 bridge 相关文档（如本文件）
3. ✅ 定义 Rust-core unified output → GitNexus-RC format 的 adapter 类型（仅在 Rust-core 内）

---

## 五、实现状态

Bridge adapter 已拆分为 3 个文件，总计 911 行，`analyze --format gitnexus-rc` 可用：

| 文件 | 行数 | 职责 |
|------|------|------|
| `crates/cli/src/bridge_format.rs` | 388 | 共享类型（`BridgeGraphOutput` / `BridgeEdge` 等）+ `group_edges_by_kind()` + 单元测试 |
| `crates/cli/src/rust_bridge.rs` | 300 | Rust 特定 bridge 转换逻辑 |
| `crates/cli/src/cangjie_bridge.rs` | 223 | Cangjie 特定 bridge 转换逻辑 |

| 步骤 | 状态 | 详情 |
|------|------|------|
| **统一 endpoint 字段名** | ✅ 完成 | `normalize_edge_endpoints()` 统一 source/target → sourceId/targetId |
| **统一 node kind 序列化** | ✅ 完成 | `partition_rust_nodes()` 从 label 推断 kind；`partition_cangjie_nodes()` 直接使用 kind |
| **实现 --format gitnexus-rc** | ✅ 完成 | `analyze --format gitnexus-rc` 支持 Rust + Cangjie |
| **schema 对齐** | ✅ 已落地 | GitNexus-RC `rust-core-bridge-adapter/`（4 文件 ~921 行）已消费 bridge JSON，含完整 10 规则验证 + 6-phase 映射 pipeline；详见 [`docs/plans/2026-05-09-gitnexus-rc-adapter-preflight.md`](../plans/2026-05-09-gitnexus-rc-adapter-preflight.md) |

### 后续步骤（需跨仓授权）

1. ~~**前端消费准备。**~~ → Bridge adapter 已在 GitNexus-RC 落地（commit `26a21b5e`），无需 Rust-core 侧额外动作
2. ~~**Bridge integration tests。**~~ → Rust-core 侧 `bridge_roundtrip` 26 tests 已覆盖（13 Rust + 13 Cangjie）
3. **Tool propagation。** GitNexus-RC Tool CLI 集成 bridge adapter（需跨仓授权）
4. **端到端验证。** bridge JSON → RC adapter → Web UI / LLM Tools 完整链路（需跨仓授权）
5. **USES edge 渲染样式。** GitNexus-RC `graph-adapter.ts` EDGE_STYLES 补充（需跨仓授权）

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.2.0 | §五 更新文件拆分详情（3 文件 911 行）、schema 对齐状态改为已落地（bridge adapter landed）、后续步骤重编号并区分已完/需授权、标题状态更新 |
| 2026-05-09 | 1.1.0 | §五 更新为实现状态（3/4 步骤已落地）；状态从 Preflight 改为 Implemented |
| 2026-05-09 | 1.0.0 | 初始 preflight：消费场景分析、Rust vs Cangjie vs GitNexus-RC 差异矩阵、可直接映射字段、需要 adapter 的差异、stop-line、下一步建议 |
