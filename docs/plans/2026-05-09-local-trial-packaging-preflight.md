# Local Trial Packaging Preflight

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Preflight
> **依赖：** Productization Phase (d016b5d..9528000)

---

## 一、动机

Productization Phase 已完成 Unified CLI Surface（analyze/quality/summary），但当前试用方式仍是 `cargo run -p gitnexus-rust-core-cli -- ...`，对非 Rust 开发者不友好。

本 slice 目标：提供最小化的本地构建脚本，使任何人都能：
1. 一键构建 gitnexus-rust-core 二进制
2. 快速跑 smoke 验证
3. 了解如何集成到自己的工作流

## 二、设计方案

### 2.1 新增文件

| 文件 | 用途 |
|------|------|
| `scripts/build.sh` | 构建二进制（release + Cangjie feature） |
| `scripts/smoke.sh` | 快速 smoke 验证（Tier 1 fixtures） |

### 2.2 build.sh 规格

- 默认构建 release binary（`cargo build --release --features tree-sitter-cangjie`）
- 支持 `--debug` flag 构建 debug binary
- 构建成功后打印二进制路径和试用命令
- 检测 `cargo` 是否可用，不可用时给出安装指引

### 2.3 smoke.sh 规格

- 跑 4 个关键路径验证：
  1. `cargo test`（no-feature）
  2. Rust analyze on portable-smoke fixture
  3. Cangjie analyze on portable-smoke fixture（feature-enabled）
  4. quality command exit code 检查
- 全部通过 → exit 0；任何失败 → exit 1
- 输出人类可读的 PASS/FAIL

### 2.4 README.md 更新

- 在 CLI Usage 章节附近新增"Local Trial"小节
- 提供 build + smoke 命令
- 提供非 Rust 开发者友好的一行命令

## 三、不做什么

- 不创建 Makefile（过度工程）
- 不发布二进制（stop-line: no commercial distribution）
- 不做 CI/CD pipeline
- 不做 `install.sh`（超出 local trial 范围）
- 不修改 Cargo.toml / workspace 结构
- 不新增依赖

## 四、Stop-line 验证

| Stop-line | 状态 |
|-----------|------|
| 不修改 GitNexus-RC | ✅ 仅改 Rust-core |
| 不修改 GitNexus-RC-Tool | ✅ |
| 不修改 live repo | ✅ |
| 不新增依赖 | ✅ |
| 不做 destructive git | ✅ |
| 不做 WebUI/MCP/HTTP | ✅ |
| 不做 production replacement | ✅ 仅为开发辅助脚本 |

## 五、Write Set

| 文件 | 操作 |
|------|------|
| `scripts/build.sh` | 新增 |
| `scripts/smoke.sh` | 新增 |
| `README.md` | 修改（新增 Local Trial 章节） |
| `docs/plans/README.md` | 修改（新增本 slice 记录） |
| `docs/plans/2026-05-09-productization-phase-closure-review.md` | 修改（更新 residual gaps 状态） |

## 六、风险

| 风险 | 级别 | 缓解 |
|------|------|------|
| scripts/smoke.sh 依赖 python3 解析 JSON | LOW | python3 在 macOS/Linux 普遍可用；fallback: 直接输出 raw JSON |
| release build 耗时较长 | LOW | 提供 --debug flag |
