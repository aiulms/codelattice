# Findings & Decisions: MCP P0 内存优化

## Requirements

- 规划并优化 MCP 高内存问题。
- 完整验证后提交并推送 CodeLattice。
- 把最新优化同步到 CodeLattice-Tool。

## Research Findings

- 现场 PID 98410 是常驻 `codelattice mcp`，运行约 12 小时，CPU 0%，physical footprint 1.7 GiB，峰值 2.8 GiB。
- `heap` 显示约 1.505 GB 活跃分配、15,069,801 个分配节点；不是单纯 Activity Monitor 口径或空闲页。
- open-nwe/backend facade 缓存文件约 165 MiB，其中 `analyze_result` 序列化约 60.3 MB，`graph_view` 约 110.4 MB。
- 缓存图含 63,706 nodes、88,392 edges、37,354 diagnostics。
- `GraphView` 将完整 node `Value` 放入 `nodes_by_id` 并再次复制到 `symbols_by_name`；完整 edge `Value` 同时复制到 `outgoing` 与 `incoming`。
- `clone_shallow` 的注释错误：`serde_json::Value::clone()` 是递归深拷贝；缓存命中还复制整个 GraphView 和 analyze result。
- project job 将包含完整 `analyzeValue` 的 artifact 再复制进 `ArtifactCache.memory`，形成额外常驻副本。
- `persistent_cache_status` 为读取元数据会 `read_to_string` 并反序列化每个完整 `PersistentCacheEntry`。实测空闲 MCP 从约 4.7 MiB 出现 1.3 GiB 峰值，完成后 live heap 仅 26 KiB但 footprint 仍约 519 MiB。
- 当前同时存在多个 MCP 进程，但只有加载大图或执行 cache status 的实例显著增大；多进程生命周期是次要放大器，不是单实例根因。
- CodeLattice-Tool 已部署二进制修改时间为 2026-07-29，早于 2026-08-02 的 P0 并行优化提交；同步确实缺失。
- CodeLattice-native 对精确 `GraphView` symbol 的 impact 结果为 HIGH：25 条 ACCESSES 边、26 个受影响节点，全部集中在 `crates/cli/src/mcp_server.rs`；没有跨文件调用扩散，但它覆盖大量 MCP 查询 consumer。
- impact baseline 处于 stale + fresh delta 状态，因此风险结论用于扩大验证范围，不作为编译或运行时证明。
- RED 已用真实指针身份确认：当前同一 symbol node 在 `nodes_by_id` 与 `symbols_by_name` 中的字符串地址不同；`clone_shallow` 前后的 node id 字符串地址也不同，证明两条路径都是递归深拷贝。
- Arc conversion 的 47 个编译点分为三类：17 个 delta-overlay mutation 点需要创建一次 Arc 后同时写两个索引；约 20 个 bounded response 构造点需要显式 clone 成 owned `Value`；其余 borrow/iterator 点只需 `Arc::as_ref`。
- GraphView 自身的 `find_symbols` / `edges_from` / `edges_to` API 仍返回 owned `Value`，因此对外行为可保持不变，深拷贝仅发生在受 limit 约束的响应边界，而不是整图索引和缓存命中路径。
- `GraphView` 内部改用 `Arc<Value>` 后，二级索引与 clone_shallow 的 allocation identity 测试已通过；当前剩余整图复制来自 `CacheEntry` 本身仍按值保存并在每次 memory hit 复制容器和 raw analyze JSON。
- `ArtifactCache::new` 的 `load_from_disk` 会遍历缓存目录、读取并反序列化所有 JSON 后立即丢弃；`get()` 本来就支持按 key 延迟加载，因此该启动扫描没有缓存命中收益，却会放大 cache status 的瞬时内存。
- `mcp_job.rs` 中 engine cache 只有两处 `cache_store`，没有任何 `cache_get` consumer；project-once 完整 artifact 常驻内存没有读收益，改成 persistent-only 不影响 facade 查询或 job_detail。
- schema v3 选择“单一 analyze JSON + 启动时重建 Arc-backed GraphView”。这牺牲少量索引重建 CPU，但消除了约 110.4 MB serialized typed snapshot、旧格式加载的重复对象峰值和保存时深拷贝。
- release 受控实测证明没有查询累积增长：fresh session 三次 overview 均 102.3 MiB，persistent session 三次均 74.5 MiB；cache status 各只增加约 0.1–0.2 MiB。
- 165 MiB legacy-format synthetic cache 的 status 扫描增量峰值只有 0.4 MiB，说明 `IgnoredAny` metadata reader 与删除 engine eager scan 后不再把大 payload 构造成 JSON 树。

## Technical Decisions

| Decision | Rationale |
|---|---|
| 优先建立共享图数据/索引，而非调 allocator 参数 | 约 1.5 GB 是活跃对象，必须消除对象复制 |
| cache status 采用 `BufReader` + 轻量反序列化结构 | 只需 header 元数据，无需构造 node/edge `Value` 树 |
| 对大型 job artifact 避免 facade warm 后保留第二份完整图 | job_detail 已保存 compact detail；完整图由 facade cache 服务查询 |
| 淘汰策略需要覆盖所有插入路径并考虑字节预算 | 当前 16-entry 计数上限对 165 MiB 单项无保护，job/persistent promotion 还可绕过统一淘汰 |

## Issues Encountered

| Issue | Resolution |
|---|---|
| 规划 skill 默认要求根目录文件，但 AGENTS.md 限制根目录写入 | 将规划文件放入允许写入的 `docs/plans/.../` 子目录 |

## Resources

- `crates/cli/src/mcp_server.rs`
- `crates/cli/src/mcp_job.rs`
- `crates/analysis-engine/src/cache.rs`
- `docs/plans/2026-05-29-facade-cache-warming-extreme-optimization.md`
- `docs/plans/2026-08-01-mcp-cache-warm-phase-a-execution-card.md`

## Visual/Runtime Findings

- 用户截图中 Activity Monitor 显示 codelattice PID 98410 内存 1.72 GB、2 threads。
- `vmmap` 与截图一致；该数值不是显示错误。
