# Graph Schema v0

> **日期：** 2026-05-03
> **类型：** 输出契约（output contract）
> **状态：** 已填充（基于 preflight 决策冻结）
> **Schema 版本：** 0.1.0
> **前置：** Rust-core ProjectModel MVP + Item/Symbol 提取 + expected-symbols P1

---

## 1. 文档定位

Graph Schema v0 是 Rust-core **独立的 graph 输出契约**。

核心原则：

- **不直接复用** GitNexus-RC 内部 graph schema（后者包含 `Community` / `Process` / `Route` / `Tool` 等分析层概念，不属于 ProjectModel 职责）。
- GitNexus-RC 后续通过 **adapter** 消费 Rust-core graph 输出。适配层放 GitNexus-RC 侧（如 `rust-core-adapter.ts`），不在 Rust-core 中实现。
- Rust-core graph 是 **ProjectModel 数据的另一种表达形式**：把 `packages/workspaces/targets/sourceOwnership/rootResolution/symbols/diagnostics` 映射为 nodes 和 edges。

与 GitNexus-RC graph schema 的兼容性约定：

- ID 格式使用冒号分隔（`NodeType:identifyingField`），与 GitNexus-RC 的 `Function:src/lib.rs:funcName` 风格兼容。
- Edge type 命名使用 `UPPER_SNAKE_CASE`，与 GitNexus-RC 一致。
- Node property 使用 `camelCase`（Rust-core serde `rename_all` 已做），与 GitNexus-RC JSON 风格一致。

---

## 2. Schema 版本

当前版本：**0.1.0**

版本策略：

- 遵循语义化版本（semver）：`MAJOR.MINOR.PATCH`。
- **Minor 版本**（0.2.0）：新增 node type / edge type / node property，不删除已有字段。Consumer 必须忽略不认识的属性（forward compatibility）。
- **Major 版本**（1.0.0）：删除字段 / 改变 ID 格式 / 改变语义。
- **0.x 阶段允许破坏性变更**，但必须在本文档 changelog 中记录 migration note。
- Schema 版本记录在 graph 输出的 `schemaVersion` 顶层字段中。

---

## 3. Output Format

MVP 使用 **JSON one-shot output**。

理由：

1. Rust-core `ProjectModelOutput` 已经是单一 JSON struct（`model.rs`），graph 输出继承相同模式。
2. 典型 repo 的 ProjectModel 数据量在 1-50MB 范围，单次 JSON 序列化无性能瓶颈。
3. NDJSON 的流式消费优势在当前 CLI-only 场景中不需要。
4. NDJSON 要求 consumer 维护行状态机，增加消费侧复杂度。

未来扩展（stop-line，本轮不实现）：

- Graph event stream / NDJSON 流式输出。
- Incremental graph update（只输出 diff）。

---

## 4. Top-level Shape

```json
{
  "schemaVersion": "0.1.0",
  "generatedAt": "2026-05-03T12:00:00Z",
  "root": {
    "repoRoot": "/path/to/repo",
    "version": "0.1.0",
    "command": "project-model inspect",
    "partial": false
  },
  "nodes": [
    { "id": "repo:.", "label": "Repository", "properties": { ... } },
    { "id": "package:Cargo.toml", "label": "Package", "properties": { ... } }
  ],
  "edges": [
    { "source": "repo:.", "target": "package:Cargo.toml", "type": "CONTAINS_PACKAGE", "properties": {} }
  ],
  "diagnostics": [
    { "id": "diag:source-outside-package::scripts/setup.rs::0", "label": "Diagnostic", "properties": { ... } }
  ],
  "stats": {
    "nodeCount": 0,
    "edgeCount": 0,
    "diagnosticCount": 0,
    "symbolCount": 0
  }
}
```

字段说明：

| 字段 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `schemaVersion` | string | 必需 | 固定 `"0.1.0"` |
| `generatedAt` | string | 必需 | ISO 8601 时间戳 |
| `root` | object | 必需 | Repository 元信息 |
| `nodes` | array | 必需 | 所有非 Diagnostic nodes |
| `edges` | array | 必需 | 所有 edges |
| `diagnostics` | array | 必需 | 所有 Diagnostic nodes（独立于 `nodes` 方便查询） |
| `stats` | object | 必需 | 统计摘要 |

---

## 5. Node Types

共 **8 种** node types，完全基于 Rust-core 已有数据模型。

| Node Label | Rust-core 来源 | 必需/可选 | 说明 |
|---|---|---|---|
| `Repository` | `repoRoot` + `version` + `partial` | 必需（恰好 1 个） | repo 根节点 |
| `Workspace` | `WorkspaceModel` | 可选 | 无 workspace 时不存在 |
| `Package` | `PackageModel` | 必需 | 每个 `Cargo.toml` 一个 |
| `Target` | `TargetModel` | 必需 | 每个 target 一个 |
| `SourceFile` | `SourceOwnership` | 必需 | 每个被扫描的 `.rs` 文件一个 |
| `Module` | `RootResolution`（`resolvedKind="module"`） | 可选 | crate 内模块路径映射 |
| `Symbol` | `Symbol` | 可选 | `--include symbols` 时存在 |
| `Diagnostic` | `Diagnostic` / `SymbolDiagnostic` | 可选 | 诊断信息 |

设计原则：

- **1:1 映射**：每种 Rust-core struct 恰好映射到一种 node type，不合并不拆分。
- **不新增 Rust-core 没有的概念**：不引入 `Crate` / `Feature` / `Dependency` 等 ProjectModel 当前不产出的 node。
- **Symbol 类型用字段区分**：`Symbol.symbolKind` 字段区分 12 种类型（function / struct / enum / trait / impl-block / method / associated-function / type-alias / const / static / macro-definition / module），不需要每种一个 node label。
- **Diagnostic 作为一等 node**：支持按 `severity` / `code` 查询，不需要遍历边。

---

## 6. Node Properties 和 ID Policy

### 6.1 ID 格式

所有 node ID 格式：`{NodeType}:{identifyingField}`

- 使用冒号分隔（与 GitNexus-RC 风格一致）。
- 所有 path 字段使用 **POSIX 相对路径**（相对于 repo root）。
- Symbol ID 直接复用 `Symbol.id`（`{packageName}::{modulePath}::{name}`）。

### 6.2 Repository

| 字段 | 必需 | 说明 |
|---|---|---|
| `repoRoot` | 必需 | repo 根目录路径 |
| `version` | 必需 | Rust-core 版本 |
| `command` | 必需 | `"project-model inspect"` |
| `partial` | 必需 | 是否 partial indexing |

**ID**: `repo:{repoRoot}`

### 6.3 Workspace

| 字段 | 必需 | 说明 |
|---|---|---|
| `manifestPath` | 必需 | `Cargo.toml` 相对路径 |
| `workspaceRoot` | 必需 | workspace 根目录相对路径 |
| `rawMembers` | 必需 | manifest 中原始 members 声明 |
| `expandedMembers` | 必需 | glob 展开后的成员路径 |

**ID**: `workspace:{manifestPath}`

### 6.4 Package

| 字段 | 必需 | 说明 |
|---|---|---|
| `name` | 必需 | package 名称 |
| `manifestPath` | 必需 | `Cargo.toml` 相对路径 |
| `packageRoot` | 必需 | package 根目录相对路径 |
| `targetCount` | 必需 | target 数量 |
| `featureNames` | 必需 | feature 列表 |
| `isWorkspaceMember` | 必需 | 是否 workspace member |
| `discoveryReason` | 必需 | 发现方式（有限集合枚举） |

**ID**: `package:{manifestPath}`

### 6.5 Target

| 字段 | 必需 | 说明 |
|---|---|---|
| `packageName` | 必需 | 所属 package 名称 |
| `name` | 必需 | target 名称 |
| `kind` | 必需 | lib / bin / test / bench / example / custom-build / unknown |
| `crateRootFile` | 必需 | crate root 文件路径 |
| `sourceRootDir` | 必需 | source 根目录路径 |

**ID**: `target:{packageName}::{name}::{kind}`

### 6.6 SourceFile

| 字段 | 必需 | 说明 |
|---|---|---|
| `sourcePath` | 必需 | `.rs` 文件相对路径 |
| `package` | 可选 | 所属 package 名称（null = 无归属） |
| `target` | 可选 | 所属 target 名称（null = ambiguous 或无归属） |
| `ownershipReason` | 必需 | 归属原因 code |
| `confidence` | 必需 | 归属置信度 (0.0-1.0) |

**ID**: `file:{sourcePath}`

### 6.7 Module

| 字段 | 必需 | 说明 |
|---|---|---|
| `sourcePath` | 必需 | 查询来源文件路径 |
| `queryPath` | 必需 | `crate::` 查询路径 |
| `resolvedPath` | 可选 | 解析结果路径（null = 解析失败） |
| `targetKind` | 可选 | 解析结果 target 类型 |
| `rootReason` | 必需 | 解析原因 code |
| `confidence` | 必需 | 解析置信度 |
| `resolvedKind` | 可选 | `"module"` / `"file"` / null |
| `crateRootFile` | 可选 | 当前 source file 的 crate root |

**ID**: `module:{sourcePath}::{queryPath}`

### 6.8 Symbol

| 字段 | 必需 | 说明 |
|---|---|---|
| `id` | 必需 | 稳定身份（`{packageName}::{modulePath}::{name}`） |
| `name` | 必需 | item 标识符 |
| `symbolKind` | 必需 | 12 种枚举值 |
| `sourcePath` | 必需 | `.rs` 文件相对路径 |
| `packageName` | 必需 | 所属 package |
| `targetName` | 可选 | 所属 target |
| `modulePath` | 可选 | crate 内 module 路径 |
| `visibility` | 必需 | 可见性枚举 |
| `parentId` | 可选 | 父 item id |
| `lineStart` | 必需 | 起始行号（1-indexed） |
| `lineEnd` | 必需 | 结束行号 |
| `genericParams` | 可选 | generic 参数原文 |
| `isAsync` | 必需 | 是否 async |
| `isUnsafe` | 必需 | 是否 unsafe |
| `isConstFn` | 必需 | 是否 const fn |
| `isPub` | 必需 | 是否 public |
| `implDetails` | 可选 | impl 块详情（嵌套对象） |

**ID**: `symbol:{id}`（直接复用 `Symbol.id`）

### 6.9 Diagnostic

| 字段 | 必需 | 说明 |
|---|---|---|
| `code` | 必需 | diagnostic code（有限集合枚举） |
| `severity` | 必需 | `"error"` / `"warning"` / `"info"` |
| `message` | 必需 | 人类可读描述 |
| `path` | 必需 | 受影响路径 |
| `confidence` | 可选 | 关联置信度 |
| `reason` | 可选 | reason code |
| `relatedPaths` | 必需 | 相关路径列表 |
| `suggestedAction` | 可选 | 修复建议 |
| `symbolId` | 可选 | 关联 symbol id（仅 `SymbolDiagnostic`） |

**ID**: `diag:{code}::{path}::{index}`

---

## 7. Edge Types

共 **8 种** edge types，以包含关系和结构映射为主。

| Edge Type | 源 → 目标 | 语义 | 必需/可选 |
|---|---|---|---|
| `CONTAINS_WORKSPACE` | `Repository` → `Workspace` | repo 包含 workspace | 可选 |
| `CONTAINS_PACKAGE` | `Repository` / `Workspace` → `Package` | repo/workspace 包含 package | 必需 |
| `HAS_TARGET` | `Package` → `Target` | package 拥有 target | 必需 |
| `OWNS_SOURCE` | `Target` / `Package` → `SourceFile` | target/package 拥有 source file | 必需 |
| `RESOLVES_TO` | `SourceFile` → `SourceFile` / `Module` | crate:: 解析结果 | 可选 |
| `DEFINES` | `SourceFile` → `Symbol` | 文件定义 symbol | 可选 |
| `HAS_PARENT` | `Symbol` → `Symbol` | symbol 父子关系（impl → method） | 可选 |
| `ANNOTATES` | `Diagnostic` → 任意 Node | diagnostic 关联到目标 node | 可选 |

设计原则：

- **不引入 CALLS / IMPORTS / EXTENDS / IMPLEMENTS**：这些是 call graph 语义，ProjectModel v0 不产出。
- **OWNS_SOURCE 目标灵活**：`SourceFile` 的 owner 可以是 `Target`（明确归属）或 `Package`（ambiguous target 时），与 `SourceOwnership.target` 的 nullable 语义对齐。
- **RESOLVES_TO 可自环**：`SourceFile` 可以 resolve 到自身（`crate::` 指向自己的 target root 文件）。
- **ANNOTATES 目标无类型约束**：`Diagnostic` 可以 annotate 任意 node type（`SourceFile`、`Package`、`Symbol` 等）。Consumer 根据 `Diagnostic.path` / `Diagnostic.symbolId` 判断实际目标。

---

## 8. Edge 详细约束

### 8.1 CONTAINS_WORKSPACE

| 属性 | 值 |
|---|---|
| Source | `Repository` |
| Target | `Workspace` |
| Required fields | 无 |
| confidence/reason | 不允许 |

Multiplicity: 每个 `Workspace` 恰好有 1 个 `CONTAINS_WORKSPACE` 入边。

### 8.2 CONTAINS_PACKAGE

| 属性 | 值 |
|---|---|
| Source | `Repository`（非 workspace repo）或 `Workspace`（workspace repo） |
| Target | `Package` |
| Required fields | 无 |
| confidence/reason | 不允许 |

Multiplicity: 每个 `Package` 恰好有 1 个 `CONTAINS_PACKAGE` 入边。

### 8.3 HAS_TARGET

| 属性 | 值 |
|---|---|
| Source | `Package` |
| Target | `Target` |
| Required fields | 无 |
| confidence/reason | 不允许 |

Multiplicity: 每个 `Target` 恰好有 1 个 `HAS_TARGET` 入边。

### 8.4 OWNS_SOURCE

| 属性 | 值 |
|---|---|
| Source | `Target`（明确归属）或 `Package`（ambiguous target 时 `target=null`） |
| Target | `SourceFile` |
| Required fields | `ownershipReason`, `confidence` |
| confidence/reason | 必需（从 `SourceOwnership` 透传） |

Multiplicity: 每个 `SourceFile` 恰好有 1 个 `OWNS_SOURCE` 入边（包括 `source-outside-package` 的低置信归属）。

### 8.5 RESOLVES_TO

| 属性 | 值 |
|---|---|
| Source | `SourceFile` |
| Target | `SourceFile`（`resolvedKind="file"`）或 `Module`（`resolvedKind="module"`） |
| Required fields | `rootReason`, `confidence` |
| confidence/reason | 必需（从 `RootResolution` 透传） |

Multiplicity: 每个 `RootResolution` 产生 1 条 `RESOLVES_TO` 边。解析失败（`resolvedPath=null`）**不产边**，而是生成 `Diagnostic`。

### 8.6 DEFINES

| 属性 | 值 |
|---|---|
| Source | `SourceFile` |
| Target | `Symbol` |
| Required fields | 无 |
| confidence/reason | 不允许（定义关系是确定性事实） |

Multiplicity: 每个 `Symbol` 恰好有 1 条 `DEFINES` 入边。

### 8.7 HAS_PARENT

| 属性 | 值 |
|---|---|
| Source | `Symbol`（子 symbol，如 method） |
| Target | `Symbol`（父 symbol，如 impl block） |
| Required fields | 无 |
| confidence/reason | 不允许（父子关系是确定性事实） |

Multiplicity: 每个 `Symbol` 最多 1 条 `HAS_PARENT` 出边（`parentId` 为 null 时无边）。

### 8.8 ANNOTATES

| 属性 | 值 |
|---|---|
| Source | `Diagnostic` |
| Target | 任意 Node（`SourceFile` / `Package` / `Target` / `Symbol` 等） |
| Required fields | 无 |
| confidence/reason | 不允许 |

目标推断规则：

- `Diagnostic.path` 存在时 → annotate 对应 `SourceFile` 或 `Package`。
- `SymbolDiagnostic.symbolId` 存在时 → annotate 对应 `Symbol`。
- 两者都存在时 → 产生 2 条 `ANNOTATES` 边。

---

## 9. Mapping Policy

Rust-core `ProjectModelOutput` → graph nodes/edges 的映射规则：

### 9.1 ProjectModel 层（必选）

| Rust-core 数据 | Graph 输出 |
|---|---|
| 1 个 `repoRoot` | 1 个 `Repository` node |
| N 个 `WorkspaceModel` | N 个 `Workspace` node + N 条 `CONTAINS_WORKSPACE` 边 |
| N 个 `PackageModel` | N 个 `Package` node + N 条 `CONTAINS_PACKAGE` 边 |
| N 个 `TargetModel` | N 个 `Target` node + N 条 `HAS_TARGET` 边 |
| N 个 `SourceOwnership` | N 个 `SourceFile` node + N 条 `OWNS_SOURCE` 边 |
| N 个 `RootResolution`（resolved） | N 条 `RESOLVES_TO` 边 |
| N 个 `RootResolution`（failed） | N 个 `Diagnostic` node |
| N 个 `Diagnostic` | N 个 `Diagnostic` node + M 条 `ANNOTATES` 边 |

### 9.2 Item/Symbol 层（`--include symbols` 时）

| Rust-core 数据 | Graph 输出 |
|---|---|
| N 个 `Symbol` | N 个 `Symbol` node + N 条 `DEFINES` 边 |
| M 个 `Symbol`（`parentId` 非 null） | M 条 `HAS_PARENT` 边 |
| N 个 `SymbolDiagnostic` | N 个 `Diagnostic` node + M 条 `ANNOTATES` 边 |

---

## 10. No-edge Policy

核心原则：**ambiguous / missing / skipped 不能生成 fake edge**。

| 场景 | 正确行为 | 错误行为 |
|---|---|---|
| SourceFile 无 package 归属 | `OWNS_SOURCE` 指向 Repository（最低置信） | 不产边 / 指向错误 package |
| SourceFile target ambiguous | `OWNS_SOURCE` 指向 Package（非 Target） | 随机选择一个 Target |
| RootResolution 解析失败 | 不产 `RESOLVES_TO` 边，生成 Diagnostic | 生成指向错误文件的边 |
| Symbol 提取失败（parse error） | 不产 `DEFINES` 边，生成 Diagnostic | 生成不完整的 Symbol node |
| cfg-gated module 可达性未知 | 不产 `RESOLVES_TO` 边 | 假设可达并产边 |

**所有 no-edge 决策必须通过 `Diagnostic` node + `ANNOTATES` edge 记录**，不允许默默跳过。

---

## 11. Stop-line

Graph Schema v0 **不做** 以下内容：

1. **不做 CALLS** — call graph 需要 import resolution + type inference + trait solving，超出 ProjectModel 职责。
2. **不做 USES / IMPORTS** — `use` 语句解析需要 module resolution，当前 `RootResolution` 只到 crate root。
3. **不做 IMPLEMENTS 最终语义** — trait implementation 的完整性需要 type inference，当前只记录 impl block 的存在。
4. **不做 external crate graph** — 依赖外部 crate 的 resolution 需要 `cargo metadata` 或网络访问。
5. **不做 macro expansion** — macro 生成代码不在 AST 中，只记录 `macro-invocation-unexpanded` diagnostic。
6. **不做 type inference / trait solving** — 需要 rust-analyzer 级别的能力，超出 heuristic 范围。
7. **不引入 graph database** — graph 输出是 JSON 文件，不写入 LadybugDB / Neo4j / 任何数据库。

---

## 12. Future Extensions

以下功能不在 v0 范围内，但 schema 设计已为它们预留扩展空间：

### 12.1 CALLS

- 需要：import resolver + call extractor + type-position reference resolution。
- Schema 扩展：新增 `CALLS` edge type（`Function` / `Method` → `Function` / `Method`），带 `confidence` / `reason` 属性。
- 预计复杂度：高（需要 full language adapter）。

### 12.2 USES / IMPORTS

- 需要：`use` 语句解析 + module declaration map + crate:: resolution。
- Schema 扩展：新增 `USES` / `IMPORTS` edge type。
- 预计复杂度：中（大部分逻辑已在 GitNexus-RC TypeScript 侧验证）。

### 12.3 IMPLEMENTS

- 需要：trait implementation 提取 + trait bound checking。
- Schema 扩展：新增 `IMPLEMENTS` edge type（`Struct` → `Trait`）。
- 预计复杂度：中（tree-sitter AST 已可见 trait impl）。

### 12.4 Graph Event Stream

- 需要：增量 graph diff 计算 + NDJSON 序列化。
- Schema 扩展：新增 `GraphEvent` 概念（`add_node` / `remove_edge` 等）。
- 预计复杂度：低（序列化格式变更）。

### 12.5 GitNexus-RC Adapter

- 需要：Rust-core graph → GitNexus-RC graph 的映射层。
- 位置：GitNexus-RC 侧（如 `rust-core-adapter.ts`），不在 Rust-core 中。
- Node label 映射：`Package` → `Module`（概念近似），`Symbol` → `Function` / `Struct` / `Enum` / `Trait` / `Impl` / `Method` 等（按 `symbolKind` 分发）。

---

## Changelog

| 日期 | 版本 | 变更 |
|---|---|---|
| 2026-05-03 | 0.1.0 | 初始填充：8 node types + 8 edge types + JSON one-shot + mapping policy + no-edge policy + stop-line |

---

## 来源

- [Rust-core Graph Schema v0 Preflight](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-03-rust-core-graph-schema-v0-preflight.md)
- [Rust-core ProjectModel 模块设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-module-design.md)
- [Rust-core model.rs](../../crates/project-model/src/model.rs)（数据模型参考）
