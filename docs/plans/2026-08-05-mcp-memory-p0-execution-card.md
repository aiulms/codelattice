# MCP P0 内存优化 — Execution Card（2026-08-05）

> 状态：Implementation and verification complete / delivery pending
> 目标：降低大型项目 MCP warm 后的常驻内存和 cache status 峰值；保持图谱语义与对外 JSON 契约。

## 1. Evidence Baseline

| Evidence | Baseline |
|---|---:|
| MCP physical footprint | 1.7 GiB |
| MCP peak footprint | 2.8 GiB |
| live malloc bytes | 1.505 GB |
| live allocation count | 15,069,801 |
| backend facade cache file | 165 MiB |
| serialized analyze result | 60.3 MB |
| serialized GraphView snapshot | 110.4 MB |
| cache status cold-process peak | 1.3 GiB |

## 2. Root Cause

1. `CacheEntry` 同时常驻完整 `analyze_result` 与 GraphView。
2. GraphView 用 `serde_json::Value` 在多个索引间做递归深拷贝；所谓 `clone_shallow` 实际仍为深拷贝。
3. project job 又把完整 `analyzeValue` artifact 放入独立的 engine memory cache。
4. persistent cache status/clear 为读取 header 而完整反序列化图内容。
5. 缓存上限按 entry 数量而不是字节预算，且部分插入路径没有统一淘汰。

## 3. Native Impact

- Target：`symbol:gitnexus-rust-core-cli::crate::mcp_server::GraphView`
- Risk：**HIGH**
- Blast radius：25 条 ACCESSES、26 个下游节点，集中在 `crates/cli/src/mcp_server.rs`。
- 决策：允许实现，但提交前必须运行完整 MCP integration suite、全量测试、native precommit 和确定性对比。

## 4. Write Set

| File | Intended change |
|---|---|
| `crates/cli/src/mcp_server.rs` | shared GraphView values/cheap clone、persistent metadata reader、统一 cache eviction/status |
| `crates/cli/src/mcp_job.rs` | project artifact 生命周期收紧（仅在证据表明确认无读路径后） |
| `crates/analysis-engine/src/cache.rs` | persistent-only store / size-aware lifecycle（若需要） |
| `crates/cli/tests/mcp_server.rs` | MCP cache/query contract regression |
| `docs/plans/2026-08-05-mcp-memory-p0*` | plan、证据、closure |
| `CHANGELOG.md` | Unreleased Performance |
| `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` | 验证通过后通过官方 promotion 脚本同步 release runtime（安装目录，不是 Git 仓库） |

## 5. Forbidden Set

- 不改 CALLS/IMPORTS edge 语义、confidence/reason、node/edge id。
- 不改 MCP/CLI 对外字段名与默认 profile。
- 不修改 open-nwe/cangjie 等 live repo，不执行 production analyze。
- 不删除现有用户缓存，不终止用户 MCP 进程。
- 不引入 unsafe、type inference、trait solving、macro expansion 或 cargo metadata。
- 不顺手重构 `calls.rs` 或其他语言分析器。

## 6. Delivery Slices

1. **Slice A — cache status metadata streaming**：先失败测试，再用 lightweight serde header + buffered reader，禁止构造 graph Value。
2. **Slice B — shared GraphView representation**：节点/边只拥有一份 Value，二级索引共享；查询 clone 只复制 Arc/容器，不递归复制 JSON。
3. **Slice C — cache lifecycle**：移除无消费者的完整 artifact 内存副本；磁盘 artifact 只在显式读取时提升到内存。
4. **Slice D — deploy closure**：release benchmark、全门禁、CodeLattice commit/push、CodeLattice-Tool binary sync/commit/push。

## 7. Stop-lines

- 每个 production slice 必须先观察对应测试 RED，再实现 GREEN。
- 同一失败最多 3 次；第 3 次后停止并重新评估架构。
- 任一 MCP 输出契约或确定性 diff 出现非预期变化：停止提交。
- `cargo fmt --check`、`git diff --check`、相关测试、全量 `cargo test`、`scripts/codelattice-precommit-check.sh` 必须全部通过。
- CodeLattice-native native review 若仍为 high/critical，提交说明必须记录风险与验证证据。

## 8. Acceptance Targets

- 受控大缓存 metadata status：不反序列化 graph payload，进程增量峰值目标 `<100 MiB`。
- GraphView 单条 node/edge 在二级索引间共享同一 allocation；测试以 `Arc::ptr_eq` 证明。
- 缓存命中不递归克隆完整 graph/analyze result；测试以共享所有权证明。
- 受控本仓 release MCP warm 后常驻与峰值均较 baseline 明显下降；重复 3 次查询无单调增长。
- 行为等价：现有 MCP tests、全量 tests、确定性输出 diff 通过。

## 9. Closure Evidence

- GraphView node/edge 在所有二级索引间共享 `Arc<Value>`；memory hit 共享 GraphView 与 analyze result 所有权。
- persistent schema v3 仅保存 analyze JSON；status/clear 只流式读取元数据，旧 schema 由 4 KiB prefix 低成本失效。
- facade/engine 使用 buffered temporary write + atomic rename；project artifact 改为 persistent-only、显式读取时才进入内存。
- 隔离 release MCP：fresh peak 108.1 MiB、persistent warm peak 74.6 MiB，连续三次查询均无单调增长。
- 165.0 MiB legacy status 的进程增量峰值为 0.4 MiB；完整 `cargo test` 退出码 0。
- native precommit 退出码 0（MCP 339/339、并发与 detect-changes smoke 通过）；最终静态风险为 `critical`，来源是中央 MCP cache 对 16 个 workspace project 的传播及 2 个 unsupported-language fixture 边界，交付以完整测试、受控基准和限定 staged write set 约束。
