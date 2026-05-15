# Unified Output Contract

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Active（Productization Slice 1）
> **来源：** gitnexus-rust-core Productization / Bridge Preparation

---

## 目的

定义 Rust + Cangjie 共用的最小 output contract，使两种语言的分析结果可被统一的 CLI 命令（`analyze` / `quality` / `summary`）包裹输出。

**设计原则：**
- Rust 和 Cangjie 不必立刻完全同构，但必须能被统一 wrapper 包住
- 统一层只做包装（wrapping），不做语义转换（translation）
- 新增类型优先放在 CLI 层或已有 crate，不引入新 crate 除非有明确必要

---

## 一、核心类型

### 1.1 LanguageAnalysisResult

统一分析结果顶层结构。`analyze` 命令的输出。

```rust
struct LanguageAnalysisResult {
    /// 分析语言标识
    language: String,          // "rust" | "cangjie" | "cpp"
    /// 项目根目录绝对路径
    root: String,
    /// 分析时间戳 (ISO 8601)
    analyzed_at: String,
    /// schema 版本
    schema_version: String,   // "v0.3" (Rust) | "v1.0.0" (Cangjie)
    /// 图概要统计
    summary: GraphSummary,
    /// 质量门结果
    quality_gates: Vec<QualityGateResult>,
    /// 完整图输出（与语言相关的具体结构）
    graph: serde_json::Value,
}
```

JSON 示例骨架：
```json
{
  "language": "rust",
  "root": "/path/to/project",
  "analyzedAt": "2026-05-09T12:00:00Z",
  "schemaVersion": "v0.3",
  "summary": {
    "nodeCount": 1524,
    "edgeCount": 2438,
    "symbolCount": 838,
    "sourceFileCount": 50,
    "packageCount": 3
  },
  "qualityGates": [
    {
      "gateName": "duplicate_nodes",
      "passed": true,
      "detail": "0 duplicate node IDs found"
    }
  ],
  "graph": { /* 语言特定 graph JSON */ }
}
```

### 1.2 GraphSummary

```rust
struct GraphSummary {
    /// 总节点数（含 Repository/Package/SourceFile/Symbol/Diagnostic 等所有类型）
    node_count: u32,
    /// 总边数（含所有 edge types）
    edge_count: u32,
    /// 符号节点数
    symbol_count: u32,
    /// 源文件节点数
    source_file_count: u32,
    /// 包节点数（Rust: Package + Target; Cangjie: Package）
    package_count: u32,
    /// 诊断节点数（可选，不存在时为 0）
    diagnostic_count: u32,
    /// CALLS / Uses 边计数（语言相关）
    call_edge_count: u32,
}
```

字段语义对齐：

| 字段 | Rust 来源 | Cangjie 来源 | C++ 来源 | 对齐状态 |
|------|-----------|-------------|----------|----------|
| `node_count` | `GraphStats.node_count` | `nodes.len()` | Graph node 计数 | ✅ 直接映射 |
| `edge_count` | `GraphStats.edge_count` | `edges.len()` | Graph edge 计数 | ✅ 直接映射 |
| `symbol_count` | `GraphStats.symbol_count` | Symbol node 计数 | Symbol node 计数 | ✅ 直接映射 |
| `source_file_count` | SourceFile node 计数 | SourceFile node 计数 | SourceFile node 计数 | ✅ 直接映射 |
| `package_count` | Package + Target node 计数 | Package node 计数 | 无（返回 0） | ⚠️ Rust 多 Target 层，Cangjie 无，C++ 无 Package 概念 |
| `diagnostic_count` | `GraphStats.diagnostic_count` | Diagnostic node 计数 | 无（返回 0） | ✅ 直接映射 |
| `call_edge_count` | `GraphStats.call_edge_count` | Uses+Accesses+Modifies edge 计数 | CALLS edge 计数 | ⚠️ 语义不同但可比 |

**注意：** `package_count` 语义差异记录在此。Rust 的 Package + Target 两层在 Cangjie 中对应单层 Package，C++ 无 Package 概念（返回 0）。统一 summary 时使用各自语言的定义，不做跨语言对齐归一化。

### 1.3 QualityGateResult

```rust
struct QualityGateResult {
    /// 门名称（snake_case identifier）
    gate_name: String,
    /// 是否通过
    passed: bool,
    /// 人类可读的详细说明
    detail: String,
}
```

支持的质量门列表（对齐 QUALITY.md）：

| gate_name | Rust 适用 | Cangjie 适用 | C++ 适用 | 判定逻辑 |
|-----------|----------|-------------|----------|---------|
| `duplicate_nodes` | yes | yes | yes | 重复 node ID 数 = 0 |
| `duplicate_edges` | yes | yes | yes | 重复 edge triple 数 = 0 |
| `dangling_source` | yes | yes | yes | 悬空 source 引用数 = 0 |
| `dangling_target` | yes | yes | yes | 悬空 target 引用数 = 0 |
| `deterministic` | yes | yes | yes | 两次运行输出完全一致 |
| `synthetic_nodes` | no | yes | no | CallableSource node 数 = 0 |
| `calls_endpoint_integrity` | yes | no | yes | 每条 CALLS edge 端点存在于 nodes |
| `external_symbol_marking` | yes | no | no | 外部 symbol node 有 `isExternal: true` |

### 1.4 Node/Edge 兼容性期望

**不强制同构，但应满足：**

1. **Node ID 策略稳定：** 同一项目同一版本多次运行得到相同 ID
2. **Edge 端点完整性：** 所有 edge 的 source/target 指向存在的 node
3. **JSON 可序列化：** 顶层必须能通过 `serde_json::to_value()` 序列化
4. **确定性输出：** 相同输入 → 相同 JSON（key order stable via BTreeMap）

当前差异保留：

| 差异点 | Rust | Cangjie | C++ |
|--------|------|---------|-----|
| Node 类型 | Repository/Workspace/Package/Target/SourceFile/Module/Symbol/Diagnostic | Repository/Package/SourceFile/Symbol/Diagnostic/CallableSource | Repository/SourceFile/Symbol |
| Edge 类型 | CONTAINS_WORKSPACE/CONTAINS_PACKAGE/HAS_TARGET/OWNS_SOURCE/DEFINES/CALLS/DESIGNATION/ACCESSES/HAS_PARENT | ContainsPackage/OwnsSource/Defines/Annotates/Uses/Accesses/Modifies/Imports | DEFINES/CALLS/INCLUDES |
| 图顶层字段 | schemaVersion/generatedAt/root/nodes/edges/diagnostics/stats | nodes/edges | nodes/edges |
| 额外统计 | call_edge_count/designation_edge_count/accesses_edge_count | 无 stats 结构 | call_edge_count |

---

## 二、CLI 输出协议

### 2.1 stdout / stderr 约定

- **stdout：** 纯 JSON，可直接管道到 `jq` / 文件
- **stderr：** 人类可读日志（进度、警告、错误）
- **日志宏：** `eprintln!()` 用于 stderr

### 2.2 退出码约定

| 退出码 | 含义 | 适用场景 |
|--------|------|----------|
| 0 | 成功 | 分析完成，所有质量门通过 |
| 1 | 失败 | 分析出错、root 不存在、语言检测失败 |
| 2 | 未确定/不适用 | 质量门不适用于该语言 |

### 2.3 `--language auto` 检测策略

简单检测规则（零依赖）：

1. 检查 `<root>/Cargo.toml` → `rust`
2. 检查 `<root>/cjpm.toml` → `cangjie`
3. 检查 `<root>` 含 `.cpp`/`.hpp`/`.cc`/`.cxx` 文件 → `cpp`
4. 两者都有 → 报错要求显式指定
5. 两者都没有 → 报错"无法检测语言"

---

## 三、Quality Gate 命令输出格式

`quality` 命令输出：

```json
{
  "language": "cangjie",
  "root": "/path/to/project",
  "overall": "pass",
  "gates": [
    {
      "gateName": "duplicate_nodes",
      "passed": true,
      "detail": "0 duplicate node IDs found"
    },
    {
      "gateName": "dangling_source",
      "passed": true,
      "detail": "0 dangling source references found"
    }
  ]
}
```

`overall` 取值：
- `"pass"` — 所有适用门通过
- `"fail"` — 至少一个适用门失败
- `"unsupported"` — 语言不支持质量门检查

### Summary 命令输出格式

`summary` 命令输出（精简版，不含完整 graph）：

```json
{
  "language": "rust",
  "root": "/path/to/project",
  "graphSummary": {
    "nodeCount": 1524,
    "edgeCount": 2438,
    "symbolCount": 838,
    "sourceFileCount": 50,
    "packageCount": 3,
    "diagnosticCount": 628,
    "callEdgeCount": 1054
  },
  "qualitySummary": {
    "total": 5,
    "passed": 5,
    "failed": 0
  }
}
```

---

## 四、实现策略

### 实现位置

- **统一类型定义：** 放在 `crates/cli/src/unified_types.rs`（新建）
- **语言检测：** 放在 `crates/cli/src/language_detect.rs`（新建）
- **CLI 集成：** 修改 `crates/cli/src/main.rs`，新增顶层子命令

### 不新增 crate 的理由

统一对象仅是 CLI 层的包装/聚合，不需要被 `project-model` 或 `cangjie` crate 内部消费。放在 CLI crate 内即可满足当前需求。

### Future Compatibility

当需要 bridge 到 GitNexus-RC 时，可以考虑：
1. 把 `unified_types.rs` 提取到共享 crate（如果 GitNexus-RC 也需要消费）
2. 或保持 CLI 内定义，通过 JSON schema 文档协调（当前推荐方案）

---

## 五、测试要求

- Rust 路径：对 fixtures/rust/portable-smoke 使用 `analyze --language rust`
- Cangjie 路径：对 fixtures/cangjie/portable-smoke 使用 `analyze --language cangjie`
- C++ 路径：对 fixtures/cpp/portable-smoke 使用 `analyze --language cpp`
- auto 路径：对 Cargo 项目 + cjpm 项目 + C++ 项目各测一次
- quality 路径：pass/fail/unsupported 三种退出码
- summary 路径：验证不含 graph inline，stats 非零

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-15 | 1.2.0 | 新增 C++ Phase A 支持：language 枚举、GraphSummary 来源、质量门适用性、Node/Edge 差异表、auto 检测策略、测试要求 |
| 2026-05-15 | 1.1.0 | 新增 MCP 缓存元数据说明（第六节） |
| 2026-05-09 | 1.0.0 | 初始版本：定义 LanguageAnalysisResult / GraphSummary / QualityGateResult / Node/Edge 兼容性期望 / CLI 输出协议 |

---

## 六、MCP 缓存元数据（v0.13）

MCP 工具在分析结果中附带缓存信号，供 AI 侧理解缓存行为。这些字段不影响 `LanguageAnalysisResult` 的 CLI 输出。

### 6.1 工具输出中的缓存信号

所有 v0.2+ 分析类工具（`codelattice_analyze`、`codelattice_summary` 等）在输出中包含：

```json
{
  "cacheHit": true,
  "cacheLayer": "memory",
  "analysisDurationMs": 0
}
```

- `cacheHit`：布尔值，本次是否命中缓存
- `cacheLayer`：命中层 `"memory"` / `"persistent"`，或未命中时为 `null`
- `analysisDurationMs`：仅未命中时返回实际分析耗时（毫秒）

### 6.2 cache_status 嵌套格式

`codelattice_cache_status` 返回双层嵌套结构：

```json
{
  "memory": {
    "entryCount": 2,
    "maxEntries": 16,
    "entries": ["..."],
    "totalHits": 5,
    "totalMisses": 2,
    "totalEvictions": 0,
    "persistentHits": 1,
    "persistentMisses": 0
  },
  "persistent": {
    "enabled": true,
    "cacheDir": "/path/to/cache-dir",
    "entryCount": 1,
    "totalSizeBytes": 24576,
    "entries": ["..."]
  }
}
```

### 6.3 失效原因（staleReasons）

缓存条目失效时返回结构化原因列表：

| staleReason | 说明 |
|-------------|------|
| `file_added` | 新源文件出现 |
| `file_removed` | 已跟踪文件消失 |
| `file_modified` | 文件 mtime 变化 |
| `manifest_changed` | Cargo.toml / cjpm.toml 等配置 hash 变化 |
| `docs_changed` | 文档文件变化 |
| `version_changed` | CodeLattice 版本升级 |
| `cache_missing` | 缓存文件不存在 |
| `cache_corrupted` | 缓存文件 JSON 解析失败 |
