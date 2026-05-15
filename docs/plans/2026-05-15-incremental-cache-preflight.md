# Incremental Cache Pack — Preflight Audit

> **日期：** 2026-05-15
> **状态：** Preflight (current state audit)
> **目标：** 为 CodeLattice MCP 分析链路增加持久化增量缓存

---

## 1. 现有缓存架构审计

### 1.1 McpCache (Process-Local)

**位置：** `crates/cli/src/mcp_server.rs:290-541`

**核心类型：**

```rust
// Cache key
struct CacheKey {
    root: String,    // canonical path
    language: String, // "rust" | "cangjie" | "arkts" | "typescript" | "auto"
    strict: bool,
}

// Cache entry
struct CacheEntry {
    analyze_result: Value,        // Full analyze JSON output
    graph_view: GraphView,        // In-memory index for queries
    created_at: Instant,
    last_used_at: Instant,
    hit_count: u64,
    analysis_duration_ms: u64,
    file_mtimes: HashMap<String, u64>,  // relative_path → mtime_ms
    root_canonical: String,
}

// Cache container
struct McpCache {
    entries: HashMap<CacheKey, CacheEntry>,
    total_hits: u64,
    total_misses: u64,
    total_evictions: u64,
}
```

**Cache key 字段：**
- `root` — canonical path (resolved via `canonicalize()`)
- `language` — requested language string
- `strict` — whether strict mode was used

**Cache hit 条件：**
1. Key match in `entries`
2. `mtimes_are_stale()` returns false (file list + mtime comparison)

**LRU eviction：**
- Max 16 entries (`CACHE_MAX_ENTRIES`)
- Evicts least-recently-used entry when full

**生命周期：**
- `McpCache::new()` in `run_mcp_server()` (line 5694)
- Lives for duration of MCP server process
- **Not persisted** — dies with the process
- **Not shared** across MCP server instances

### 1.2 Stale Detection (mtime-based)

**函数：** `scan_file_mtimes()` (line 327) + `mtimes_are_stale()` (line 369)

**扫描的文件扩展名：**
```rust
let extensions = ["rs", "cj", "toml", "json"];
```

**⚠️ 问题：** 缺少 `.ets`、`.ts`、`.tsx`、`.js`、`.jsx`、`.md` 扩展名。ArkTS 和 TypeScript 项目变更不会被检测。

**跳过的目录：**
- Hidden dirs (starting with `.`)
- `target/`
- `node_modules/`

**Stale 判定逻辑：**
1. File count mismatch → stale (files added/removed)
2. Any cached mtime != current mtime → stale (file modified)
3. All match → fresh

**⚠️ 问题：** 没有区分 stale reason，只有 bool。没有 manifest/docs 分离检测。

### 1.3 GraphView (In-Memory Query Layer)

**位置：** `crates/cli/src/mcp_server.rs:1465-1740`

```rust
struct GraphView {
    nodes_by_id: HashMap<String, Value>,
    symbols_by_name: HashMap<String, Vec<Value>>,
    outgoing: HashMap<String, Vec<Value>>,
    incoming: HashMap<String, Vec<Value>>,
    diagnostics: Vec<Value>,
    language: String,
    root: String,
    doc_scanner: Option<Arc<DocScanner>>,
}
```

- Built from `analyze` JSON output
- Indexes nodes by id, symbols by lowercase name, edges by source/target
- Has `clone_shallow()` for cheap cloning
- `DocScanner` attached after build

### 1.4 DocScanner

**位置：** `crates/cli/src/mcp_server.rs:2343-2440`

- Scans `.md` files under project root
- Extracts sections, references, code fences, symbol refs, path refs
- Built fresh each time `get_or_analyze` runs (line 461)
- **Not cached separately** — rebuilt with each cache miss
- **Not checked for staleness** — md file changes not in `scan_file_mtimes`

### 1.5 Tools Using Cache

**通过 `cache.get_or_analyze()` 使用缓存的 tools (12个):**
- `codelattice_analyze` (line 794)
- `codelattice_symbol_context` (line 1753)
- `codelattice_calls_from` (line 1998)
- `codelattice_calls_to` (line 2145)
- `codelattice_impact_preview` (line 3249)
- `codelattice_query_graph` (line 3539)
- `codelattice_project_overview` (line 3693)
- `codelattice_rename_preview` (line 3977)
- `codelattice_changed_symbols` (line 4516)
- `codelattice_production_assist` (line 4560)
- `codelattice_cache_prewarm` (line 5196)
- `codelattice_compare_runs` (line 4958)

**不使用缓存的 tools (10个):**
- `codelattice_quality` — runs subprocess directly
- `codelattice_summary` — runs subprocess directly
- `codelattice_smoke` — runs script
- `codelattice_graph_overview` — runs subprocess directly
- `codelattice_unresolved_report` — runs subprocess directly
- `codelattice_symbol_search` — runs subprocess directly
- `codelattice_export_bridge` — runs subprocess directly
- `codelattice_repo_registry` — no analysis needed
- `codelattice_cache_status` — reports cache state
- `codelattice_cache_clear` — clears cache

### 1.6 Cache Meta Fields

每个使用缓存的 tool 输出包含：
```json
{
  "cacheHit": true|false,
  "cacheKey": "canonical_path:language:strict",
  "analysisDurationMs": 123,  // only on miss
  "cachedAtMs": 456           // only on hit (age in ms)
}
```

### 1.7 Feature Gating

```rust
fn check_language_feature(language: &str) -> Result<(), Value>
```
- Checks cfg features at compile time
- `cangjie` → `tree-sitter-cangjie`
- `arkts` → `tree-sitter-arkts`
- `typescript` → `tree-sitter-typescript`

**⚠️ 问题：** Cache key 不包含 enabled features。同一 binary 如果 features 不同（不同编译），cache entries 可能不兼容。但因为是 process-local，同一 process 内 features 不变，所以当前不构成问题。持久化后需要考虑。

---

## 2. 缺口总结

| 缺口 | 影响 | 优先级 |
|------|------|--------|
| 无持久化 — process 死亡后全部 miss | MCP server 重启后冷启动慢 | **P0** |
| scan_file_mtimes 缺少 .ets/.ts/.tsx/.js/.jsx/.md | ArkTS/TypeScript/docs 变更不被检测 | **P0** |
| 无 stale reason — 只有 bool | AI 不知道为什么 cache miss | **P1** |
| 无 manifest 级别检测 | Cargo.toml/oh-package.json5 变更不触发 stale | **P1** |
| 无 version 检测 | CodeLattice 升级后旧 cache 可能不兼容 | **P1** |
| DocScanner 每次重建 | 大项目 docs scan 慢 | **P2** |
| compact 参数不在 cache key 中 | compact 只影响输出 shape，不分析，当前正确 | ✅ OK |

---

## 3. 设计方向

### 3.1 Two-Layer Cache

```
Layer 1: Process-Local (existing McpCache, enhanced)
  - In-memory, fast
  - Same behavior as now + stale reason
  
Layer 2: Persistent (new)
  - Disk-based, survives restart
  - ${TMPDIR}/codelattice-cache/ or CODELATTICE_CACHE_DIR
  - Stores: fingerprint + analyze JSON + GraphView serializable form
  - Validated via fingerprint before loading
```

### 3.2 Fingerprint Model

```
ProjectFingerprint {
  root_canonical: String,
  language: String,
  // NOT: strict, compact (output-only, don't affect analysis)
  version: String,           // CodeLattice version
  features: String,          // enabled features marker
  
  source_files: HashMap<String, FileEntry>,  // .rs/.cj/.ets/.ts/.tsx/.js/.jsx/.json/.toml/.md
  manifest_hash: Option<String>,   // hash of Cargo.toml/Cargo.lock/oh-package.json5/package.json/tsconfig.json
  docs_hash: Option<String>,       // hash of .md files list + mtimes
}

FileEntry {
  mtime_ms: u64,
  size: u64,                // optional fast check
}
```

### 3.3 Stale Reasons

```
enum StaleReason {
  FileAdded(Vec<String>),
  FileRemoved(Vec<String>),
  FileModified(Vec<String>),
  ManifestChanged,
  DocsChanged,
  VersionChanged,
  FeatureChanged,
  CacheMissing,
  CacheCorrupted,
}
```

### 3.4 Persistent Cache Entry (on disk)

```
cache_file = ${CACHE_DIR}/${safe_hash}.json
{
  "schemaVersion": 1,
  "fingerprint": { ... },
  "analyzeResult": { ... },     // full analyze JSON
  "createdAt": "2026-05-15T12:00:00Z",
  "root": "/path/to/project",
  "language": "rust"
}
```

### 3.5 Changed Symbols / Production Assist Interaction

- `changed_symbols` 需要当前 graph + git diff
- 如果 graph stale → 重新 analyze（现有逻辑通过 `get_or_analyze` 已保证）
- git diff 始终 fresh（每次调用时重新运行）
- **关键：** persistent cache 的 graph 可以安全用于 changed_symbols，因为 stale detection 确保文件未变

---

## 4. 基线验证结果

- HEAD: `fb3719c` on `master`
- Working tree: clean (untracked: .agents/, .arts/, .claude/, .codeartsdoer/)
- `cargo fmt --check`: ✅ clean
- `git diff --check`: ✅ clean
- `cargo test --test mcp_server`: 83/83 passed
- `cargo test`: all passed
