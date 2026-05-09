# Consumer Contract — GitNexus Rust-core 本地试用输出契约

> **日期：** 2026-05-09
> **版本：** v1.0.0
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
      { "sourceId": "...", "targetId": "...", "kind": "CALLS" }
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

## 五、Exit Codes 总览

| 命令 | Exit 0 | Exit 1 | Exit 2 |
|------|--------|--------|--------|
| `analyze` | 分析成功（无 `--strict` 时） | 分析失败 / root 不存在 / 语言检测失败 | — |
| `analyze --strict` | 分析成功 + 所有质量门通过 | 质量门失败 / 分析失败 | — |
| `quality` | 所有门通过 | 至少一门失败 | （当前不使用） |
| `summary` | 成功 | 失败 | — |

---

## 六、使用示例

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

## 七、已知限制（by design）

| 限制 | 影响 | 原因 |
|------|------|------|
| Rust/Cangjie graph 结构不同 | `graph` 字段不可跨语言直接比较 | 语言差异，不做归一化 |
| Bridge format `packageId` 部分为空 | 部分 Rust source-file 无法通过 edge 链路解析 | 仅限 OWNS_SOURCE + HAS_TARGET 两跳；复杂 workspace 结构可能缺失 |
| Cangjie bridge `fileId`/`parentId` 为 null | Cangjie 符号不携带文件信息 | Cangjie 图结构设计中不包含这些属性 |
| `--language auto` 仅检查 manifest | 不做语言启发式/内容检测 | 第一版设计：simple detection only |
| 非 JSON 格式不支持 | 只有 JSON stdout | MVP scope |
| Cangjie 需 `--features tree-sitter-cangjie` | 默认编译不含 Cangjie 支持 | feature gate，需单独启用 |

---

## 八、不承诺的保证

以下行为不在 consumer contract 中保证：

1. **Sort order stability：** JSON 数组中元素顺序可能变化，消费侧应按 ID 或 name 做 set membership 检查
2. **Node ID 格式：** ID 格式视为内部实现细节，不保证跨版本稳定（但保证同版本多次运行稳定）
3. **Schema 向前兼容：** `schemaVersion` 可能在未来 minor 版本升级
4. **Production readiness：** 当前为本地试用阶段，不做 production SLA 承诺

---

## 九、Stop-line

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
| 2026-05-09 | 1.0.0 | 初始版本：三种输出格式定义、字段稳定性、exit codes、使用示例、已知限制、stop-line |
