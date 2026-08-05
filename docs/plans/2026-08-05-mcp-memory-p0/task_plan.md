# Task Plan: MCP P0 内存优化与 CodeLattice-Tool 同步

## Goal

在不改变图谱语义和 MCP/CLI 输出契约的前提下，显著降低大型 Rust 项目 warm 后的 MCP 常驻内存与 cache status 峰值，完成验证、提交、推送，并把最新 release 二进制同步到 CodeLattice-Tool。

## Current Phase

Phase 5

## Phases

### Phase 1: Preflight 与执行卡

- [x] 复核现场内存、堆对象和磁盘缓存组成
- [x] 执行 CodeLattice-native 影响分析
- [x] 冻结 write set、forbidden set、stop-line 和量化验收标准
- **Status:** complete

### Phase 2: RED 测试

- [x] 为 GraphView 共享节点/边、真正浅克隆添加失败测试
- [x] 为 persistent cache status 流式元数据读取添加失败测试
- [x] 为内存 cache hit 共享 GraphView/analyze result 添加失败测试
- [x] 为大 artifact 不重复常驻添加失败测试
- **Status:** complete

### Phase 3: 分步实现

- [x] 消除 GraphView 内部和查询返回路径的深拷贝
- [x] 修复 persistent cache status/clear 全量反序列化
- [x] 收紧 facade/engine 大 artifact 生命周期
- **Status:** complete

### Phase 4: 验证与 closure review

- [x] 相关测试、cargo fmt、git diff check
- [x] release 构建与受控内存基准
- [x] 全量 cargo test
- [x] native precommit（最终原子写入补丁后的复核）
- [x] 更新 execution card 和 CHANGELOG
- **Status:** complete

### Phase 5: 提交、推送、同步工具仓

- [ ] 提交 CodeLattice 并 push gitcode master
- [x] 确认 CodeLattice-Tool 为本地安装目录、不是 Git 仓库，使用本仓官方 promotion 脚本同步
- [ ] 同步 release 二进制并验证版本/哈希/冒烟
- [ ] 记录安装备份与 manifest sourceCommit（安装目录无独立 commit/push）
- **Status:** pending

## Key Questions

1. 能否以共享所有权替换深拷贝，同时保持所有 MCP 输出逐字段兼容？
2. 哪一层缓存可以只持久化而不保留完整图的内存副本？
3. 旧 schema v2 大缓存如何以低峰值兼容读取或安全失效？
4. CodeLattice-Tool 的同步与发布门禁具体要求是什么？

## Decisions Made

| Decision | Rationale |
|---|---|
| 重新打开原 Phase A，但目标改为内存而非延迟 | 现场证据证明 GraphView/JSON 深拷贝是 1.7 GiB 常驻和 2.8 GiB 峰值的主因 |
| 先写失败测试，再逐刀实现 | 避免大范围表示层重构破坏 50 个 MCP 工具的行为 |
| 计划文件放在 `docs/plans/2026-08-05-mcp-memory-p0/` | 满足 planning-with-files，同时遵守 AGENTS.md 只允许写 `docs/` 等目录的边界 |
| GraphView 切片按 HIGH risk 管理 | native impact 显示 26 个同文件下游消费者；必须依靠 MCP 集成测试和输出确定性验证，而不能只跑局部单测 |

## Errors Encountered

| Error | Attempt | Resolution |
|---|---:|---|
| `cargo test ... -- --exact` matched 0 tests because the unit test name includes its module path | 1 | Re-run with a substring filter and no `--exact` |
| First Arc conversion compile exposed 47 type mismatches across GraphView consumers | 1 | Treat compiler output as blast-radius inventory; convert mutation sites to shared Arc and clone to owned Value only at bounded response boundaries |
| First consumer repair left 19 type mismatches | 2 | Finished bounded-output and iterator adaptations; `cargo check -p gitnexus-rust-core-cli` now passes |
| 上一轮全量测试输出会话关闭后无法取回退出码 | 1 | 重新运行完整 `cargo test`，退出码 0 |

## Notes

- 不删除或清空用户现有缓存，不终止现有 MCP 进程。
- 不修改 open-nwe 源码，不执行其生产分析；基准复用已存在缓存和本仓/fixture 受控场景。
- 不改变 CALLS/IMPORTS/confidence/reason 等图谱语义。
