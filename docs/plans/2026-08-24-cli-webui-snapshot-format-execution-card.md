# 执行卡：桌面端「分析项目」第一步 —— CLI 原生 --format webui-snapshot（方案 B）

日期：2026-08-24 · 任务来源：用户任务书《打通 CodeLattice 桌面工作台的「分析项目」链路》
决策记录：用户先选 C（Tauri 内转换），实现完成后确认 B 才是长期治本（单一事实源），
C 已整体回滚（src-tauri 回到 15 passed 基线），转换逻辑与测试移植进 core。

## 问题

桌面端把 `codelattice analyze --format json` 的原始输出（schemaVersion 0.3.0）原样
发布为工作台快照，前端只消费 `webui.snapshot.v1`，字段形状全面不一致。

## 方案

core 新增 `--format webui-snapshot`：analyze 结果在 CLI 内部直接转换为
webui.snapshot.v1 输出，转换器成为唯一事实源（后续 Python 脚本可退役，另立任务）。

- 新模块 `crates/cli/src/webui_snapshot.rs`：
  `convert_analyze_result(analyze_json, tool_version) -> Value`
  移植自 C 方案已验证的实现（其本身移植自 scripts/codelattice-snapshot-gen.py 的
  桌面消费子集）：summary / graph（节点选择 + kind 映射 + relationKey sha256）/
  moduleGraph / limitations / insights。stats 全部实算，不硬编码；不产 dangling 边。
- `crates/cli/src/lib.rs`：Analyze 的 format 白名单加 `webui-snapshot`；
  print_analyze_result 增加 format 参数，webui-snapshot 时强制 full profile
  （非 full 直接报错退出，不做静默忽略），序列化 result 后走转换器输出。
- 依赖：cli crate 加 `sha2 = "0.10"`（relationKey §6.1；0.10.9 及全部依赖已在本地
  cargo 缓存，离线可构建）。
- toolVersion 取 `env!("CARGO_PKG_VERSION")`（CLI 自身版本）。

桌面端（第二步前的最小接入）：
- `analyzer.rs`：子进程改调 `--format webui-snapshot`；发布前 peek 产物
  schemaVersion，非 `webui.snapshot.v1`（如 workspaceAutoEntry 多项目清单）按 Failed
  上抛并写明原因——只保留这个几行的检测，不保留 C 的完整转换器。

## Write set

- `crates/cli/src/webui_snapshot.rs`（新增）
- `crates/cli/src/lib.rs`（format 白名单 + print 路径 + mod 声明）
- `crates/cli/Cargo.toml`（+sha2）、`Cargo.lock`（构建自动更新）
- `crates/cli/tests/webui_snapshot_format.rs`（新增集成测试）
- `apps/desktop/src-tauri/src/analyzer.rs`（格式切换 + schema 守卫 + 测试）
- 本文件

## Forbidden set / stop-line

- 不碰 `crates/project-model/src/calls.rs`；不改任何语言 adapter 的节点/边产出
- 不改 fixtures、不改 webui 契约文档、不改 open-nwe、不提交 git
- 工作区里大量未提交的前序改动（moduleGraph 等）不属于本任务，一律不碰
- webui-snapshot 不做路径 redact（redact 是 fixture 发布场景需求，由 Python 脚本继续负责）

## TDD 顺序

webui_snapshot.rs 先测试 + stub 跑红 → 实现 → CLI 集成测试（fixtures/rust/portable-smoke
端到端）→ 桌面端接入 → 验收。

## 验收

- `cargo test`（根 workspace 全量，含新增转换器测试与 CLI 集成测试）
- `cargo fmt --check`、`git diff --check`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --no-default-features`
- `cd apps/desktop && npx vitest run`（126 不回退）、`npx tsc --noEmit`
- 手动 smoke：`target/debug/codelattice analyze --root fixtures/rust/portable-smoke
  --language rust --format webui-snapshot` 产出可被 meta_of 读取（rootLabel/language 正确）
