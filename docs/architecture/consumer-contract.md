# Consumer Contract — GitNexus Rust-core 本地试用输出契约

> **日期：** 2026-05-09
> **版本：** v1.2.0
> **状态：** Active
> **受众：** 前端消费侧、AI workflow 集成、下游工具链开发者

---

## 目的

定义 GitNexus Rust-core CLI 的对外输出契约，使消费侧可以稳定地解析、验证、集成三种 JSON 输出格式。

**Stop-line 明确：**
- 不替换 GitNexus-RC 现用工具
- 不接 MCP / HTTP / WebUI
- 不修改 GitNexus-RC / GitNexus-RC-Tool
- 不做 production release packaging

---

## 零、字段稳定性分类体系

为确保下游消费侧（GitNexus-RC adapter、AI workflow、脚本）接入时不重新猜字段语义，所有 Bridge JSON 字段分为三个稳定性层级：

### Tier 1 — Stable（可直接消费）

这些字段语义稳定、跨版本不变，消费侧可以直接使用，无需 adapter 转换：

| 字段路径 | 类型 | 语义 |
|---------|------|------|
| `schemaVersion` | string | 源 graph schema 版本 |
| `generatedAt` | string | ISO 8601 生成时间戳 |
| `language` | string | `"rust"` 或 `"cangjie"` |
| `root` | string | 项目根目录绝对路径 |
| `repository.id` | string | 仓库节点 ID |
| `repository.path` | string | 仓库路径 |
| `packages[].id` | string | 包节点 ID |
| `packages[].name` | string | 包名称 |
| `packages[].manifestPath` | string | manifest 文件路径 |
| `sourceFiles[].id` | string | 源文件节点 ID |
| `sourceFiles[].path` | string | 源文件相对路径 |
| `symbols[].id` | string | 符号节点 ID（**格式不跨版本稳定**，见 §4.6） |
| `symbols[].name` | string | 符号名称（人类可读） |
| `symbols[].kind` | string | 具体符号类型（如 `function`/`struct`/`enum`/`Class`/`Init`） |
| `edges.*[].sourceId` | string | 边源端点（归一化为 sourceId） |
| `edges.*[].targetId` | string | 边目标端点（归一化为 targetId） |
| `edges.*[].kind` | string | 边原始类型名称 |
| `edges.*[].confidence` | Option<f64> | 边置信度（Rust 有值，Cangjie 为 null） |
| `edges.*[].reason` | Option<String> | 边解析原因（Rust 有值，Cangjie 为 null） |
| `stats.*` | u32 | 统计字段（与数组实际计数一致） |

### Tier 2 — Adapter-Required（需 adapter 映射后消费）

这些字段在 bridge JSON 中存在，但 GitNexus-RC KnowledgeGraph 语义不同，必须通过 adapter 转换：

| 字段路径 | Bridge 值 | RC 期望 | 转换方式 |
|---------|----------|---------|---------|
| `symbols[].kind` | `"function"` / `"struct"` / `"Class"` 等 | `NodeLabel` 枚举（`Function` / `Struct` / `Class` 等） | Adapter 做 kind→NodeLabel 映射 |
| `edges.*[].kind` | `"CALLS"` / `"DEFINES"` / `"uses"` 等 | `RelationshipType` 枚举（`CALLS` / `DEFINES` / `USES` 等） | Adapter 做 kind→RelationshipType 映射 |
| `symbols[].id` | `"sym:path::Name"` 格式 | `"{filePath}:{NodeLabel}:{name}"` 格式 | Adapter 重建 ID |
| `sourceFiles[].id` | `"file:src/lib.rs"` 格式 | `"{filePath}:File:{filename}"` 格式 | Adapter 重建 ID |
| `packages[].id` | `"pkg:Cargo.toml"` 格式 | `"{manifestPath}:Package:{name}"` 格式 | Adapter 重建 ID |
| `edges.*[].kind`（字段名） | `"kind"` | `"type"` | Adapter 重命名字段 |
| `symbols[].name` | 顶层字段 | `properties.name` | Adapter 嵌套到 properties |
| `sourceFiles[].path` | 顶层字段 | `properties.filePath` | Adapter 嵌套到 properties |

### Tier 3 — Intentionally-Unstable（不保证跨版本稳定）

这些字段语义可能变化，消费侧不应依赖其格式或存在性：

| 字段路径 | 不稳定性来源 | 建议 |
|---------|------------|------|
| `symbols[].id` 格式 | ID 策略可能调整（如 arity 后缀、namespace 前缀） | 不要做字符串解析或正则匹配 ID |
| `symbols[].fileId` | Rust 有值，Cangjie 为 null；格式可能变化 | 不要跨语言假设其存在 |
| `symbols[].parentId` | 同上 | 同上 |
| `sourceFiles[].packageId` | Rust 通过 edge traversal 解析，部分文件可能为 null | 不要假设所有文件都有 packageId |
| `diagnostics[]` | 内容格式未标准化（Rust/Cangjie 不同） | 仅统计计数，不做内容解析 |
| `edges.*[].properties` | RC 不消费，bridge 保留源数据 | 不做跨系统依赖 |
| `symbols[].properties` | 同上 | 不做跨系统依赖 |
| `packages[]` 含 target 节点 | Rust bridge 将 target 也放入 packages（RC adapter 视为 metadata-only） | 按 `language` 区分处理 |

---

## 一、三种输出格式

Rust-core CLI 提供三种 JSON 输出格式，通过 `--format` flag 切换：

| 格式 | `--format` 值 | 命令 | 用途 |
|------|--------------|------|------|
| **Unified JSON** | `json`（默认） | `analyze` | 完整语言分析结果，含 graph + quality gates + summary |
| **Quality JSON** | `json`（默认） | `quality` | 质量门检查结果，含 exit code |
| **Bridge JSON** | `gitnexus-rc` | `analyze` | GitNexus-RC 兼容格式，按节点/边类型显式分组 |

**Summary 命令**（`summary`）输出精简统计，不含完整 graph。

---

## 二、Unified JSON（`--format json`）

### 2.1 输出结构

```json
{
  "language": "rust",
  "root": "/absolute/path/to/project",
  "analyzedAt": "2026-05-09T12:00:00Z",
  "schemaVersion": "v0.3",
  "summary": {
    "nodeCount": 16,
    "edgeCount": 25,
    "symbolCount": 6,
    "sourceFileCount": 2,
    "packageCount": 2,
    "diagnosticCount": 0,
    "callEdgeCount": 4
  },
  "qualityGates": [
    {
      "gateName": "duplicate_nodes",
      "passed": true,
      "detail": "0 duplicate node IDs found"
    }
  ],
  "graph": { /* 语言特定的完整 graph */ }
}
```

### 2.2 字段稳定性

| 字段 | 稳定级别 | 说明 |
|------|---------|------|
| `language` | ✅ 稳定 | `"rust"` 或 `"cangjie"` |
| `root` | ✅ 稳定 | 绝对路径字符串 |
| `analyzedAt` | ✅ 稳定 | ISO 8601 UTC 时间戳 |
| `schemaVersion` | ✅ 稳定 | Rust: `"v0.3"`, Cangjie: `"v1.0.0"` |
| `summary.*` | ✅ 稳定 | 7 个统计字段，见下文 |
| `qualityGates[]` | ✅ 稳定 | gateName/passed/detail 三元组 |
| `graph` | ⚠️ 语言相关 | Rust 和 Cangjie 的 graph 内部结构不同 |

### 2.3 GraphSummary 字段

| 字段 | 类型 | 语义 |
|------|------|------|
| `nodeCount` | u32 | 总节点数（含 Repository/Package/SourceFile/Symbol/Diagnostic） |
| `edgeCount` | u32 | 总边数（含所有 edge types） |
| `symbolCount` | u32 | 符号节点数 |
| `sourceFileCount` | u32 | 源文件节点数 |
| `packageCount` | u32 | 包节点数（Rust: Package+Target; Cangjie: Package） |
| `diagnosticCount` | u32 | 诊断节点数 |
| `callEdgeCount` | u32 | CALLS（Rust）或 Uses（Cangjie）边数 |

**注意：** `packageCount` 在 Rust 和 Cangjie 之间语义不同（Rust 包含 Target 层）。消费侧不应对此做跨语言对齐。

### 2.4 QualityGateResult 字段

| 字段 | 类型 | 语义 |
|------|------|------|
| `gateName` | string | 门名称（snake_case） |
| `passed` | bool | 是否通过 |
| `detail` | string | 人类可读说明 |

**Rust 适用门（7 个）：**
- `duplicate_nodes` / `duplicate_edges` / `dangling_source` / `dangling_target`
- `deterministic` / `calls_endpoint_integrity` / `external_symbol_marking`

**Cangjie 适用门（6 个）：**
- `duplicate_nodes` / `duplicate_edges` / `dangling_source` / `dangling_target`
- `deterministic` / `synthetic_nodes`

---

## 三、Quality JSON（`quality` 命令）

### 3.1 输出结构

```json
{
  "language": "rust",
  "root": "/path/to/project",
  "overall": "pass",
  "gates": [
    {
      "gateName": "duplicate_nodes",
      "passed": true,
      "detail": "0 duplicate node IDs found"
    }
  ]
}
```

### 3.2 Exit Codes

| 退出码 | 含义 | 触发条件 |
|--------|------|----------|
| 0 | 全部通过 | `overall = "pass"` |
| 1 | 质量门失败 | `overall = "fail"`，至少一个门未通过 |
| 2 | 语言不支持 | 语言不支持质量门检查（当前不使用） |

### 3.3 overall 取值

| 值 | 含义 |
|----|------|
| `"pass"` | 所有适用门通过 |
| `"fail"` | 至少一个适用门失败 |
| `"unsupported"` | 语言不支持（当前不使用） |

---

## 四、Bridge JSON（`--format gitnexus-rc`）

### 4.1 输出结构

```json
{
  "schemaVersion": "v0.3",
  "generatedAt": "2026-05-09T12:00:00Z",
  "language": "rust",
  "root": "/path/to/project",
  "repository": {
    "id": "repo:...",
    "path": "/path/to/project"
  },
  "packages": [
    { "id": "pkg:...", "name": "my-crate", "manifestPath": "Cargo.toml" }
  ],
  "sourceFiles": [
    { "id": "file:...", "path": "src/main.rs", "packageId": "pkg:..." }
  ],
  "symbols": [
    {
      "id": "sym:...",
      "name": "my_func",
      "kind": "function",
      "fileId": "file:...",
      "parentId": null,
      "properties": {}
    }
  ],
  "edges": {
    "calls": [
      { "sourceId": "...", "targetId": "...", "kind": "CALLS", "confidence": 0.75, "reason": "direct call via import" }
    ],
    "defines": [
      { "sourceId": "...", "targetId": "...", "kind": "DEFINES" }
    ],
    "uses": [],
    "accesses": [],
    "designations": [],
    "imports": [],
    "contains": [],
    "owns": [],
    "annotates": [],
    "other": []
  },
  "diagnostics": [],
  "stats": {
    "nodeCount": 16,
    "edgeCount": 25,
    "symbolCount": 6,
    "sourceFileCount": 2,
    "packageCount": 2,
    "diagnosticCount": 0,
    "callEdgeCount": 4
  }
}
```

### 4.2 结构保证

Bridge 格式提供以下结构保证（由 `bridge_roundtrip` 测试套件验证）：

| 保证项 | 验证方式 |
|--------|---------|
| 顶层字段完整 | `schemaVersion`/`repository`/`packages`/`sourceFiles`/`symbols`/`edges`/`diagnostics`/`stats` 全部存在 |
| 端点完整性 | 所有 edge 的 `sourceId`/`targetId` 指向已知 node-like ID |
| 端点字段归一化 | 统一使用 `sourceId`/`targetId`（非 `source`/`target`） |
| 统计一致性 | `stats` 各字段与实际数组计数一致 |
| 无空路径 | `sourceFiles[].path` 非空字符串 |
| 无空符号名 | `symbols[].name` 非空字符串 |
| 确定性输出 | 两次运行相同输入产出相同 JSON |

### 4.3 字段稳定性

| 字段 | 稳定级别 | 说明 |
|------|---------|------|
| `schemaVersion` | ✅ 稳定 | 来自源 graph |
| `language` | ✅ 稳定 | `"rust"` / `"cangjie"` |
| `repository.id` / `repository.path` | ✅ 稳定 | 仓库标识 |
| `packages[].id` / `name` / `manifestPath` | ✅ 稳定 | 包列表 |
| `sourceFiles[].id` / `path` | ✅ 稳定 | 源文件列表 |
| `sourceFiles[].packageId` | ⚠️ 部分 | Rust 通过 edge traversal 解析，部分源文件可能为 null |
| `symbols[].id` / `name` / `kind` | ✅ 稳定 | 符号列表 |
| `symbols[].fileId` / `parentId` | ⚠️ 部分 | Rust 有此信息，Cangjie 为 null |
| `edges.*[]` | ✅ 稳定 | 按类型分组的边数组 |
| `edges.*[].sourceId` / `targetId` / `kind` | ✅ 稳定 | 归一化后统一格式 |
| `stats.*` | ✅ 稳定 | 统计字段与数组计数一致 |
| `diagnostics[]` | ⚠️ 部分 | Cangjie 当前可能为空 |

### 4.4 Edge 类型分组

| 分组 | 包含的边类型（Rust） | 包含的边类型（Cangjie） |
|------|---------------------|------------------------|
| `calls` | CALLS | — |
| `defines` | DEFINES | defines |
| `uses` | — | uses |
| `accesses` | ACCESSES | accesses |
| `designations` | DESIGNATION | — |
| `imports` | — | imports |
| `contains` | CONTAINS_PACKAGE / CONTAINS_WORKSPACE / HAS_TARGET | containsPackage / containsWorkspace |
| `owns` | OWNS_SOURCE | ownsSource |
| `annotates` | ANNOTATES | annotates |
| `other` | HAS_PARENT / RESOLVES_TO | modifies / hasParent / resolvesTo |

---

## 五、Rust vs Cangjie Bridge 输出差异

消费侧必须按 `language` 字段区分处理，两种语言的 bridge 输出有以下结构性差异：

### 5.1 节点差异

| 维度 | Rust | Cangjie |
|------|------|---------|
| workspace 节点 | ✅ 有（CONTAINS_WORKSPACE edge） | ❌ 无 |
| target 节点（lib/bin） | ✅ 有（packages 数组中） | ❌ 无 |
| module 节点 | ❌ 跳过（不计入 symbols） | ❌ 无此概念 |
| diagnostic 节点 | ✅ diagnostics 数组有内容 | ⚠️ 当前可能为空 |
| 符号 fileId/parentId | ✅ 有值 | ❌ 均为 null |
| 符号 packageId | ⚠️ 通过 edge traversal 两跳解析 | ✅ 直接来自 properties.packageId |
| packages 语义 | Package + Target 都算 package | 仅 Package 算 package |
| 符号 kind 来源 | `properties.symbolKind` | `properties.kind` |

### 5.2 边差异

| 维度 | Rust | Cangjie |
|------|------|---------|
| CALLS edges | ✅ 有（`calls` 分组） | ❌ 无（用 `uses` 分组表示 call reference） |
| DESIGNATION edges | ✅ 有（`designations` 分组） | ❌ 无 |
| imports edges | ❌ 无 | ✅ 有（`imports` 分组） |
| modifies edges | ❌ 无 | ✅ 有（`other` 分组） |
| edge confidence/reason | ✅ 从源 graph 提取 | ❌ 均为 null（源数据不提供） |
| edge 端点字段名 | 归一化前为 source/target | 原生 sourceId/targetId（无需归一化） |
| CONTAINS_PACKAGE | ✅ 有 | ✅ containsPackage |
| CONTAINS_WORKSPACE | ✅ 有 | ❌ 无 |
| HAS_TARGET | ✅ 有 | ❌ 无 |

### 5.3 Stats 语义差异

| 统计字段 | Rust 语义 | Cangjie 语义 |
|---------|----------|-------------|
| `packageCount` | Package + Target 节点总数 | 仅 Package 节点数 |
| `callEdgeCount` | CALLS 边数 | Uses 边数（含 call reference + type reference） |
| `diagnosticCount` | diagnostics 数组长度 | 从 nodes 中统计 kind=diagnostic 的数量 |

消费侧不应跨语言比较 packageCount 或 callEdgeCount。

---

## 六、Node ID 不稳定边界

### 6.1 核心原则

**Bridge JSON 的 raw node ID（`symbols[].id`、`sourceFiles[].id`、`packages[].id`）仅在单次 bridge 输出内部有效，不保证跨版本、跨语言、跨系统的互查能力。**

### 6.2 已知 ID 格式（仅供参考，不做契约）

| 节点类型 | Rust ID 示例 | Cangjie ID 示例 |
|---------|-------------|----------------|
| Repository | `repo:<name>` | `repo:<name>` |
| Package | `pkg:<manifest-path>` | `pkg:<name>` |
| Target | `target:<name>::<kind>` | — |
| SourceFile | `file:<relative-path>` | `sf:<relative-path>` |
| Symbol | `sym:<path>::<name>` | `sym:<path>:<kind>:<name>` |
| Init（Cangjie） | — | `sym:<path>:Init:<Owner>.init#<arity>` |

### 6.3 消费侧安全规则

1. **不要在 bridge raw ID 和 GitNexus-RC KnowledgeGraph ID 之间做字符串相等比较。** 两种系统使用不同的 ID 策略（RC adapter 会重建 ID）。
2. **不要对 ID 做正则解析或格式假设。** ID 格式可能因 arity suffix、namespace prefix、特殊字符转义等调整。
3. **跨系统关联应通过 adapter 转换后的 ID。** 只有经过 `map-to-gitnexus.ts` 转换的 ID 才能用于 RC KnowledgeGraph 查询。
4. **同版本内多次运行的 ID 是稳定的。** 同一输入、同一版本的两次运行产生相同的 ID（已通过 determinism 测试验证）。

---

| 命令 | Exit 0 | Exit 1 | Exit 2 |
|------|--------|--------|--------|
| `analyze` | 分析成功（无 `--strict` 时） | 分析失败 / root 不存在 / 语言检测失败 | — |
| `analyze --strict` | 分析成功 + 所有质量门通过 | 质量门失败 / 分析失败 | — |
| `quality` | 所有门通过 | 至少一门失败 | （当前不使用） |
| `summary` | 成功 | 失败 | — |

---

---

## 七、Exit Codes 总览

| 命令 | Exit 0 | Exit 1 | Exit 2 |
|------|--------|--------|--------|
| `analyze` | 分析成功（无 `--strict` 时） | 分析失败 / root 不存在 / 语言检测失败 | — |
| `analyze --strict` | 分析成功 + 所有质量门通过 | 质量门失败 / 分析失败 | — |
| `quality` | 所有门通过 | 至少一门失败 | （当前不使用） |
| `summary` | 成功 | 失败 | — |

---

## 八、使用示例

### 6.1 Rust 项目分析（Unified JSON）

```bash
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json

# 用 jq 提取统计
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke --language rust --format json 2>/dev/null \
  | jq '{nodes: .summary.nodeCount, symbols: .summary.symbolCount, edges: .summary.edgeCount}'

# strict 模式（CI/CD）
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke --language rust --format json --strict
```

### 6.2 Cangjie 项目分析（需 feature）

```bash
# 需要 tree-sitter-cangjie feature
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/cangjie/portable-smoke \
  --language cangjie \
  --format json

# jq 提取 quality gates
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/cangjie/portable-smoke --language cangjie --format json 2>/dev/null \
  | jq '.qualityGates[] | {gate: .gateName, pass: .passed}'
```

### 6.3 Bridge 格式（GitNexus-RC 兼容）

```bash
# Rust bridge
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc 2>/dev/null \
  | jq '{lang: .language, files: .sourceFiles | length, symbols: .symbols | length, edges: .stats.edgeCount}'

# Cangjie bridge
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/cangjie/portable-smoke --language cangjie --format gitnexus-rc 2>/dev/null \
  | jq '{lang: .language, files: [.sourceFiles[].path], symbols: [.symbols[].name]}'

# 端点完整性验证（jq）
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke --language rust --format gitnexus-rc 2>/dev/null \
  | jq -e '
    ( [.repository.id] + [.packages[].id] + [.sourceFiles[].id] + [.symbols[].id] ) as $ids |
    [.edges | to_entries | .[].value[]? | select(.sourceId as $s | $ids | index($s) | not) | "dangling source: \(.sourceId)"] |
    if length > 0 then error("dangling sources found") else "all endpoints OK" end
  '
```

### 6.4 质量门检查

```bash
# Rust quality — exit code 0 表示全通过
cargo run -p gitnexus-rust-core-cli -- quality \
  --root fixtures/rust/portable-smoke --language rust
echo "Exit: $?"

# Cangjie quality
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- quality \
  --root fixtures/cangjie/portable-smoke --language cangjie
echo "Exit: $?"
```

### 6.5 统计摘要

```bash
cargo run -p gitnexus-rust-core-cli -- summary \
  --root fixtures/rust/portable-smoke --language rust --format json 2>/dev/null \
  | jq '{lang: .language, nodes: .graphSummary.nodeCount, quality: .qualitySummary}'
```

### 6.6 一键构建 + 验证

```bash
# 构建 release 二进制
./scripts/build.sh

# 完整 smoke 验证（含 bridge format）
./scripts/smoke.sh

# 快速 smoke（跳过 cargo test）
./scripts/smoke.sh --quick
```

---

## 九、已知限制（by design）

| 限制 | 影响 | 原因 |
|------|------|------|
| Rust/Cangjie graph 结构不同 | `graph` 字段不可跨语言直接比较 | 语言差异，不做归一化 |
| Bridge format `packageId` 部分为空 | 部分 Rust source-file 无法通过 edge 链路解析 | 仅限 OWNS_SOURCE + HAS_TARGET 两跳；复杂 workspace 结构可能缺失 |
| Cangjie bridge `fileId`/`parentId` 为 null | Cangjie 符号不携带文件信息 | Cangjie 图结构设计中不包含这些属性 |
| `--language auto` 仅检查 manifest | 不做语言启发式/内容检测 | 第一版设计：simple detection only |
| 非 JSON 格式不支持 | 只有 JSON stdout | MVP scope |
| Cangjie 需 `--features tree-sitter-cangjie` | 默认编译不含 Cangjie 支持 | feature gate，需单独启用 |

---

## 十、不承诺的保证

以下行为不在 consumer contract 中保证：

1. **Sort order stability：** JSON 数组中元素顺序可能变化，消费侧应按 ID 或 name 做 set membership 检查
2. **Node ID 格式：** ID 格式视为内部实现细节，不保证跨版本稳定（但保证同版本多次运行稳定）
3. **Schema 向前兼容：** `schemaVersion` 可能在未来 minor 版本升级
4. **Production readiness：** 当前为本地试用阶段，不做 production SLA 承诺

---

## 十一、Stop-line

以下行为绝不在 Rust-core 中发生：

- 不替换 GitNexus-RC / GitNexus-RC-Tool 作为默认工具
- 不添加 MCP server / HTTP API / WebUI
- 不修改 GitNexus-RC runtime / schema / adapter
- 不修改 live repo（open-nwe / cangjie / warp / openfang）
- 不做 commercial distribution / release packaging
- 不新增外部依赖（无预授权）

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.2.0 | 消费者契约固化：新增 §零 字段稳定性三级分类（Stable / Adapter-Required / Intentionally-Unstable）；新增 §五 Rust vs Cangjie 输出差异表（节点/边/stats）；新增 §六 Node ID 不稳定边界与安全规则；章节重新编号 |
| 2026-05-09 | 1.1.0 | Cross-repo consumer dry-run：Bridge edge 新增 confidence/reason 顶层字段；symbol kind 填入具体类型（非通用 "symbol"）；新增 `docs/architecture/gitnexus-rc-consumer-dry-run.md` 兼容性报告 |
| 2026-05-09 | 1.0.0 | 初始版本：三种输出格式定义、字段稳定性、exit codes、使用示例、已知限制、stop-line |
