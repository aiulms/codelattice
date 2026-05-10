# MCP v0.3 Local Cache Pack — Preflight

> **日期：** 2026-05-11
> **版本：** v0.3.0
> **状态：** Preflight

---

## 一、性能瓶颈复盘

当前 MCP server 每个 tool call 都执行完整 analyze subprocess：

| 场景 | 分析耗时 | 原因 |
|------|---------|------|
| fixture (c1-same-module, 7 nodes) | 3-8s | subprocess 启动 + 分析 |
| CodeLattice self (1809 nodes) | 60-90s | 大型项目分析 |
| AI 高频交互 (10+ calls/session) | 30s-15min | 累积 |

瓶颈根因：`build_graph_view()` → `run_analyze_subprocess()` → 每次调用 spawn 新进程。

## 二、Cache Object 设计

### Key

```rust
struct CacheKey {
    root: String,        // canonical path (from PathBuf::canonicalize)
    language: String,    // "rust" | "cangjie" | "auto"
    strict: bool,
    include_graph: bool, // analyze mode: compact vs full
}
```

`CacheKey` 实现了 `Hash` + `Eq`，用作 `HashMap<CacheKey, CacheEntry>` 的 key。

### Value (CacheEntry)

```rust
struct CacheEntry {
    analyze_result: Value,    // 完整 analyze JSON output
    graph_view: GraphView,    // 预构建的 GraphView
    created_at: Instant,      // 缓存创建时间
    last_used_at: Instant,    // 最后使用时间
    hit_count: u64,           // 命中次数
    analysis_duration_ms: u64,// 原始分析耗时
}
```

### Cache Stats

```rust
struct CacheStats {
    total_hits: u64,
    total_misses: u64,
}
```

### 主结构

```rust
struct McpCache {
    entries: HashMap<CacheKey, CacheEntry>,
    stats: CacheStats,
}
```

## 三、Invalidation 策略

| 策略 | 实现 | 说明 |
|------|------|------|
| 手动 clear | `codelattice_cache_clear` tool | 按 root/language 过滤清除 |
| 进程退出自动清 | Rust drop | 进程内缓存，不持久化 |
| 无 TTL | 默认无过期 | 简单可靠，AI client 可按需 clear |
| 无 mtime/commit check | 不检查文件变更 | 避免复杂度，client 可 clear 后重新分析 |

## 四、Cache 使用范围

### 使用 cache 的 tools（通过 `get_or_analyze` helper）

| Tool | Cache 用途 |
|------|-----------|
| `codelattice_analyze` | 直接返回 cached analyze_result（或新分析） |
| `codelattice_quality` | 从 cached result 提取 quality gates |
| `codelattice_summary` | 从 cached result 提取 summary |
| `codelattice_graph_overview` | 从 cached GraphView 提取 overview |
| `codelattice_unresolved_report` | 从 cached result 提取 unresolved |
| `codelattice_symbol_search` | 从 cached GraphView 搜索 |
| `codelattice_symbol_context` | 从 cached GraphView 查上下文 |
| `codelattice_calls_from` | 从 cached GraphView BFS |
| `codelattice_calls_to` | 从 cached GraphView BFS |
| `codelattice_impact_preview` | 从 cached GraphView 计算 impact |
| `codelattice_query_graph` | 从 cached GraphView 查询 |
| `codelattice_project_overview` | 从 cached GraphView+result 生成 overview |
| `codelattice_repo_registry` | 从 cached result 提取 status |
| `codelattice_rename_preview` | 从 cached GraphView 查找 references |

### 不使用 cache 的 tools

| Tool | 原因 |
|------|------|
| `codelattice_smoke` | 运行测试脚本，不是分析 |
| `codelattice_export_bridge` | 需要写入 /tmp，每次应生成新文件（但可复用 analyze） |
| `codelattice_cache_status` | 查询 cache 本身 |
| `codelattice_cache_clear` | 清除 cache |

### export_bridge 特殊处理

export_bridge 可复用 cached analyze_result（避免重新分析），但每次仍需写入新文件。使用 `get_or_analyze` 获取 analyze_result，然后执行 bridge 格式转换和文件写入。

## 五、Implementation Plan

### 5.1 新增 struct/impl

1. `CacheKey` — `root: String, language: String, strict: bool, include_graph: bool`，derive `Hash, Eq, PartialEq`
2. `CacheEntry` — 包含 `analyze_result: Value, graph_view: GraphView, created_at: Instant, last_used_at: Instant, hit_count: u64, analysis_duration_ms: u64`
3. `McpCache` — `entries: HashMap<CacheKey, CacheEntry>, total_hits: u64, total_misses: u64`

### 5.2 新增 helper

```rust
fn get_or_analyze(
    cache: &mut McpCache,
    root: &Path,
    language: &str,
    strict: bool,
    include_graph: bool,
) -> Result<(CacheMeta, &GraphView, &Value), Value>
```

返回 `(CacheMeta, &GraphView, &Value)`，其中：
- `CacheMeta { cache_hit: bool, analysis_duration_ms: u64, cached_at: Option<Instant> }`

逻辑：
1. Build `CacheKey` from params
2. If key exists in cache: increment hit_count, update last_used_at, return hit
3. If miss: run `run_analyze_subprocess`, build `GraphView`, store, return miss

### 5.3 Handler 改造

每个需要分析的 handler 改为：
1. 接收 `&mut McpCache` 参数
2. 调用 `get_or_analyze(cache, root, language, strict, false)` 
3. 在输出中附加 `cacheHit` 和 `analysisDurationMs`（仅 miss 时显示 duration）

### 5.4 Server loop 改造

```rust
pub fn run_mcp_server() -> Result<(), String> {
    let mut cache = McpCache::new();
    // ...
    loop {
        // ...
        if let Some(response) = handle_request(&request, &mut cache) {
            // ...
        }
    }
}
```

`handle_request` 和所有 handler 函数签名增加 `&mut McpCache` 参数。

### 5.5 Cache tools

**`codelattice_cache_status`**:
- input: `{ root?: string, language?: string }` (both optional, for filtering)
- output: `{ entryCount, entries: [...], totalHits, totalMisses }`

**`codelattice_cache_clear`**:
- input: `{ root?: string, language?: string }` (both optional, for filtering)
- output: `{ clearedCount, remainingCount }`

### 5.6 tools_list 更新

总工具数 16 → 18。

## 六、Safety

1. **Path deny 在 cache 前执行** — `validate_root_path()` 仍为第一步
2. **Root canonicalization** — cache key 使用 canonical path
3. **不缓存 denied path** — denied path 在 validate 阶段即返回 error
4. **Read-only** — cache 不引入任何写操作
5. **No new dependencies** — 仅使用 `std::collections::HashMap` + `std::time::Instant`

## 七、Test / Dogfood Plan

### 测试覆盖

1. `tools_list` 包含 18 个工具
2. 同 root/language 连续两次 `project_overview`：第一次 miss，第二次 hit
3. `symbol_context` 在 `project_overview` 后命中 cache
4. `cache_status` 显示正确的 entryCount/hitCount
5. `cache_clear` 清除后下一次调用 miss
6. 不同 language/root 不共用 cache
7. `includeGraph` false/true 不混用
8. denied path 不进入 cache
9. smoke tool 不依赖/污染 cache
10. feature-gated Cangjie cache 行为

### Dogfood

1. 新增 cache_status / cache_clear 调用
2. 重复调用验证 cache hit
3. 记录第一次/第二次调用耗时

## 八、风险

| 风险 | 缓解 |
|------|------|
| 函数签名全面增加 `&mut McpCache` | 大量函数签名变更，但都是机械性增加参数 |
| GraphView lifetime 与 cache 引用 | 返回 `&GraphView` 引用，需注意 lifetime；或 clone（便宜） |
| includeGraph=true vs false 缓存区分 | CacheKey 包含 include_graph 字段 |
| 测试函数签名变更 | 测试中的 `handle_*()` 调用需要传入 `&mut McpCache` |
