# MCP v0.3 Local Cache — Closure Review

> **日期：** 2026-05-11
> **版本：** v0.3.0
> **状态：** Complete

---

## 一、交付物

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/cli/src/mcp_server.rs` | 新增 `CacheKey`, `CacheEntry`, `McpCache` 结构体；`get_or_analyze()` 返回 3-tuple；`merge_cache_and_result()` helper；8 个 v0.2 handler 注入 `cacheHit`/`analysisDurationMs`；2 个新 handler（cache_status, cache_clear）；tools_list 更新至 18 个工具；serverInfo version 0.3.0 |
| `crates/cli/tests/mcp_server.rs` | 更新 tools count 断言（16→18）；新增 10 个 cache 测试；总测试数 37 |
| `scripts/mcp-dogfood.sh` | 更新至 20 检查项（18 tools + cache hit 验证） |
| `scripts/mcp-local-client-smoke.sh` | 新增 2 个 cache tool 检查；更新 tools count 至 18 |

### 文档变更

| 文件 | 变更 |
|------|------|
| `docs/architecture/mcp-v0-contract.md` | 新增 §3.17 cache_status、§3.18 cache_clear、cache signal 说明；更新版本号至 v0.3.0 |
| `docs/architecture/mcp-local-client-setup.md` | 更新 tools 表至 18 个；更新版本号 |

---

## 二、验收结果

| 检查项 | 结果 |
|--------|------|
| `cargo build` | ✅ 编译通过（仅预存 warning） |
| `cargo test -p gitnexus-rust-core-cli --test mcp_server` | ✅ 37/37 pass |
| `bash scripts/mcp-dogfood.sh` | ✅ 20/20 pass |
| `bash scripts/mcp-local-client-smoke.sh` | ✅ 9/9 pass（1 skip） |
| 18 tools via `tools/list` | ✅ |
| cacheHit=False on first call | ✅ 验证 |
| cacheHit=True on second call | ✅ 验证 |
| Cross-tool cache reuse (calls_from → symbol_context) | ✅ 验证 |
| cache_clear 清空后 re-analyze 为 miss | ✅ 验证 |

---

## 三、Cache 设计决策

| 决策 | 选择 | 原因 |
|------|------|------|
| Cache key | `root + language + strict` | 分析结果由这三个参数唯一确定 |
| Cache scope | 进程内 | 最简单，无序列化/反序列化开销 |
| No TTL | 无过期时间 | MCP server 生命周期通常为单个 session |
| No disk | 无持久化 | 避免文件 I/O 和 stale cache 问题 |
| Cache miss 信号 | `cacheHit: false, analysisDurationMs: N` | 让调用者知道首次分析耗时 |
| Cache hit 信号 | `cacheHit: true`（无 duration） | 缓存命中时无分析，不报告 duration |
| v0.2 handler strict | 固定 `false` | 所有 graph intelligence 工具使用相同 strict 值，确保跨工具缓存复用 |
| analyze strict 默认 | `true` | 保持向后兼容；与 v0.2 工具的 strict 不同意味着 analyze 缓存不会与 v0.2 工具共享 |

---

## 四、Known Limitations

1. **analyze 与 v0.2 工具缓存隔离**：`codelattice_analyze` 默认 `strict=true`，v0.2 工具使用 `strict=false`，导致缓存 key 不同。先调用 analyze 不会让后续 v0.2 工具命中缓存，反之亦然。
2. **无 TTL / 自动失效**：如果源码在 MCP server 运行期间被修改，缓存不会自动失效。需手动调用 `cache_clear`。
3. **无 mtime 检查**：缓存不检查源文件修改时间。
4. **clone_shallow 开销**：每次缓存命中时 clone GraphView（Arc 共享内部数据），有少量内存开销。

---

## 五、文件行数

| 文件 | 行数 |
|------|------|
| `crates/cli/src/mcp_server.rs` | ~2650 |
| `crates/cli/tests/mcp_server.rs` | ~1600 |

---

## 六、下一步 (v0.4 候选)

- 可选：统一 analyze 和 v0.2 工具的 strict 默认值
- 可选：mtime-based cache invalidation
- 可选：LRU eviction（当缓存条目过多时）
