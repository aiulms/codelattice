# Progress Log: MCP P0 内存优化

## Session: 2026-08-05

### Phase 1: Preflight 与执行卡

- **Status:** complete
- **Started:** 2026-08-05 09:30 +08:00
- Actions taken:
  - 完成 PID、vmmap、heap、缓存文件组成和源码数据流诊断。
  - 确认 GraphView/serde_json 深拷贝为主因，cache status 全量解析为独立峰值放大器。
  - 读取 systematic-debugging、planning-with-files、test-driven-development 技能说明。
  - 创建持久化任务计划、发现记录和进度日志。
  - 运行精确 GraphView native impact；结果 HIGH，影响集中在 mcp_server.rs 的 26 个 consumer。
  - 冻结 execution card；进入 RED 测试阶段。
- Files created/modified:
  - `docs/plans/2026-08-05-mcp-memory-p0/task_plan.md`
  - `docs/plans/2026-08-05-mcp-memory-p0/findings.md`
  - `docs/plans/2026-08-05-mcp-memory-p0/progress.md`
  - `docs/plans/2026-08-05-mcp-memory-p0-execution-card.md`

### Phase 2: RED 测试

- **Status:** complete
- Actions taken:
  - 添加 GraphView 二级索引共享 allocation 与 clone_shallow 共享 allocation 的测试。
  - 添加 persistent cache status 只读 header、忽略 graph payload 结构的集成测试。
  - 首次精确过滤错误地匹配 0 tests；已识别为测试名称包含 module path，将改用 substring filter。
  - 观察 GraphView secondary-index sharing RED：node allocation 指针不相等。
  - 观察 clone_shallow RED：原图与 clone 的 JSON string allocation 指针不相等。
  - 第一轮 GREEN 将 GraphView/PersistentGraphViewSnapshot 改为 `Arc<Value>`，并加入 metadata-only reader。
  - 首次编译暴露 47 个 consumer 类型不匹配；这是 HIGH-risk blast radius 的具体清单，将按“索引内部共享、输出边界才深拷贝”统一修正。
  - 用短格式编译清单把 47 点归类为 mutation / bounded output / borrow iterator 三类，未发现需要跨文件 API 改动。
  - 第一轮 consumer 修正后编译错误从 47 降到 19；剩余均为 bounded response `Value` clone 或 iterator borrow 适配。
  - 完成剩余 consumer 适配；`cargo check -p gitnexus-rust-core-cli` 通过（仅既有 warning）。
  - GraphView secondary-index sharing 与 clone_shallow 两个测试均 GREEN。
  - persistent cache status metadata-only 集成测试 GREEN；header 读取不再构造 graph payload。
  - 下一刀进入 outer cache ownership RED：验证重复 memory hit 共享 GraphView 容器和 analyze result。
  - memory-hit allocation identity 测试观察 RED：两次命中的 GraphView key 指针不同。
  - 三个 persistent typed snapshot 新策略测试均观察 RED：status/hit/job 仍报告 `persistent_typed_graph`。
  - engine persistent-only 测试先因缺少 API 编译失败，确认大 artifact 生命周期能力尚不存在。
- Files created/modified:
  - 待记录。

### Phase 3: 分步实现

- **Status:** complete
- Actions taken:
  - `CacheEntry` 的 raw analyze JSON 与 GraphView 容器改为 `Arc` 共享；manifest/docs freshness 移到独立小字段，避免为内部 metadata 深拷贝 raw JSON。
  - memory stale-delta 使用 copy-on-write 索引容器，底层 node/edge JSON 继续共享。
  - persistent schema 升级为 v3；新文件只保存 analyze JSON，不再保存更大的 typed GraphView snapshot。
  - 新缓存使用 borrowed serializer + `BufWriter`，避免保存时构造完整 `Value` clone 和大 JSON String。
  - 旧 schema/version 先读取 4 KiB prefix 后失效，避免为了淘汰旧 165 MiB 文件而完整反序列化。
  - `ArtifactCache` 删除启动时“读入并立即丢弃所有 JSON”的无收益扫描；project artifact 改为 persistent-only，显式 `get` 时才按需提升到内存。
  - 所有本阶段 RED 测试已转 GREEN，CLI/engine 编译通过。

### Phase 4: 验证与 closure review

- **Status:** complete
- Actions taken:
  - 定向回归通过：analysis-engine 10/10、GraphView sharing 2/2、memory-hit sharing 1/1、persistent cache 11/11、project-job persistence 1/1。
  - release build 通过。
  - 隔离缓存、本仓 release 实测：fresh analyze peak 108.1 MiB，完成后 101.5 MiB；连续三次 project overview 稳定在 102.3 MiB，cache status 后 102.5 MiB。
  - 跨进程 persistent warm：peak 74.6 MiB，连续三次查询稳定在 74.5 MiB，status 后 74.6 MiB。
  - 165.0 MiB synthetic legacy cache status：idle 4.3 MiB、peak/after 4.7 MiB，增量仅 0.4 MiB，334 ms；满足 `<100 MiB` 增量目标。
  - facade 与 engine persistent writer 均改为临时文件写完后原子 rename，避免进程中断留下半写缓存；三组精确持久化回归通过。
  - 最终完整 `cargo test` 退出码 0；所有 workspace tests 与 doc-tests 通过。
  - `cargo fmt --all` 已执行；最终 native precommit 退出码 0：productization 22/22、MCP 339/339、并发 smoke 与 detect-changes smoke 均通过。
  - native detect-changes 最终风险为 `critical`：中央 MCP cache 修改传播至 16 个 workspace project，并命中 2 个 unsupported-language fixture 边界；以全量测试、精确缓存回归、受控内存基准和限定 staged write set 作为提交约束。

### Phase 5: 提交、推送与工具同步

- **Status:** in_progress
- Actions taken:
  - 确认 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` 是已安装 runtime 目录而非 Git 仓库，无独立 GitHub/GitCode 凭证或提交步骤。
  - 复核本仓 `scripts/promote-to-local-tool.sh`：将全语言 release 构建、备份现有安装、写 manifest，并执行 doctor/self-test。

## Test Results

| Test | Input | Expected | Actual | Status |
|---|---|---|---|---|
| Runtime baseline | PID 98410 via vmmap/heap | Separate live heap from allocator retention | 1.505 GB live, 1.7 GiB footprint, 2.8 GiB peak | PASS |
| Cache composition | existing backend cache via jq | Quantify duplicated representations | 60.3 MB raw + 110.4 MB GraphView | PASS |
| Native impact | exact GraphView symbol | Establish blast radius | HIGH; 26 same-file consumers | PASS |
| GraphView secondary index RED | unit fixture | shared node/edge allocation | pointer mismatch | EXPECTED FAIL |
| GraphView clone RED | unit fixture | clone shares JSON allocation | pointer mismatch | EXPECTED FAIL |
| GraphView sharing GREEN | unit fixture | shared node/edge + shallow clone allocations | 2 passed | PASS |
| Persistent status metadata RED | opaque graph payload | status still reports entry metadata | entry count 0 | EXPECTED FAIL |
| Persistent status metadata GREEN | opaque graph payload | status still reports entry metadata | 1 passed | PASS |
| CLI compile after Arc migration | cli crate | all GraphView consumers type-check | passed with existing warnings | PASS |
| Memory-hit ownership RED | two memory hits | shared GraphView/result containers | graph key pointers differ | EXPECTED FAIL |
| Memory-hit ownership GREEN | two memory hits | shared GraphView/result containers | allocation pointers equal | PASS |
| Persistent snapshot policy RED | status/hit/job | rebuilt shared graph, no typed snapshot | all 3 still typed | EXPECTED FAIL |
| Persistent snapshot policy GREEN | status/hit/job | rebuilt shared graph, no typed snapshot | 3 passed | PASS |
| Artifact persistent-only RED | engine unit | no independent memory residency | missing API compile failure | EXPECTED FAIL |
| Artifact persistent-only GREEN | engine unit | disk hit + entryCount remains 0 until get | passed | PASS |
| Release self fresh warm | isolated cache, CodeLattice root | no monotonic growth | peak 108.1 MiB; repeats 102.3/102.3/102.3 | PASS |
| Release self persistent warm | new MCP process | bounded cross-session promotion | peak 74.6 MiB; repeats flat | PASS |
| Large metadata status | synthetic 165 MiB legacy entry | incremental peak <100 MiB | +0.4 MiB; 334 ms | PASS |
| Full workspace tests | `cargo test` | all tests and doc-tests pass | exit 0 | PASS |
| Native precommit | full project governance | all checks complete | exit 0; MCP 339/339; static risk critical | PASS WITH RECORDED RISK |

## Error Log

| Timestamp | Error | Attempt | Resolution |
|---|---|---:|---|
| 2026-08-05 09:36 | `cargo test ... -- --exact` matched 0 tests | 1 | 去掉 `--exact`，用唯一 substring 过滤 |
| 2026-08-05 09:41 | Arc conversion compile: 47 mismatched consumer types | 1 | 收集短格式完整清单，机械区分 mutation/borrow/output 三类修正 |
| 2026-08-05 10:03 | 第一轮 consumer 修正仍有 19 个 mismatch | 2 | 完成 bounded response clone 与 iterator borrow 修正，第三次编译通过 |

## 5-Question Reboot Check

| Question | Answer |
|---|---|
| Where am I? | Phase 5 native gate and delivery |
| Where am I going? | CodeLattice commit/push → installed Tool promotion and smoke |
| What's the goal? | 降低 MCP 大项目常驻/峰值内存并同步 CodeLattice-Tool |
| What have I learned? | 共享所有权、单一持久化表示和按需 artifact 加载可同时消除常驻副本与查询峰值 |
| What have I done? | 四个内存切片、release 基准与全量测试完成，进入最终治理和交付 |
