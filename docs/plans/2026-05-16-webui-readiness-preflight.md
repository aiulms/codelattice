# WebUI Readiness / Snapshot Contract Pack — Preflight

> **日期：** 2026-05-16
> **状态：** Pre-flight complete → execution done
> **目标：** 为未来 CodeLattice WebUI 准备稳定的数据契约和验证基础设施

---

## 一、任务定义

### 1.1 目标

不做完整 WebUI 实现，只做 **readiness pack**：

1. 定义 `CodeLatticeWebSnapshotV1` JSON contract
2. 规划 5 个 MVP 视图（Dashboard/Explore/Impact/Cleanup/Release Review）
3. 实现 snapshot 生成脚本
4. 生成 fixture snapshots
5. 实现 smoke 验证

### 1.2 硬边界（来自 AGENTS.md + 任务 spec）

| 边界 | 规则 |
|------|------|
| 只改 CodeLattice repo | 不碰 GitNexus-RC / Tool / CodeLattice-Tool |
| 不新增前端框架 | 无 React/Vue/Svelte/Tauri/Electron |
| 不引入包管理 | 无 npm/pnpm/yarn |
| 不改 MCP 字段语义 | 只 additive 新增 |
| 不运行 promote | 不部署到 CodeLattice-Tool |
| 不改真实项目源码 | 只用 fixtures |

---

## 二、Execution Card

### Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `docs/webui/README.md` | 新增 | WebUI readiness 总览 |
| `docs/webui/webui-mvp.md` | 新增 | MVP 5 视图详细规格 |
| `docs/webui/webui-snapshot-contract.md` | 新增 | V1 JSON contract 完整定义 |
| `scripts/webui-snapshot.sh` | 新增 | Snapshot 生成脚本（bash+python stdlib） |
| `scripts/webui-snapshot-smoke.sh` | 新增 | Smoke 验证脚本 |
| `fixtures/webui-snapshots/rust-portable-smoke.snapshot.json` | 新增 | Rust fixture snapshot |
| `fixtures/webui-snapshots/typescript-portable-smoke.snapshot.json` | 新增 | TypeScript fixture snapshot |
| `README.md` | 修改 | 增加 WebUI Readiness 小节 |
| `CHANGELOG.md` | 修改 | 增加 Unreleased 条目 |
| `docs/plans/README.md` | 修改 | 增加本 pack 索引 |
| `docs/plans/2026-05-16-webui-readiness-preflight.md` | 新增 | 本文档 |
| `docs/plans/2026-05-16-webui-readiness-closure.md` | 新增 | Closure review |

### Forbidden Set

- ❌ 不创建任何前端项目文件（HTML/CSS/JS/TS/Vue/Svelte）
- ❌ 不修改 `crates/cli/src/mcp_server.rs`
- ❌ 不修改 `crates/cli/src/main.rs`
- ❌ 不运行 `cargo add` 或引入新 crate 依赖
- ❌ 不修改 `.claude/` 目录外的任何 IDE/AI client 配置
- ❌ 不修改 GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool 仓库
- ❌ 不执行 `scripts/promote-to-local-tool.sh`

### Stop-line

如果以下任一条件触发，立即停止并记录：

1. snapshot 脚本需要调用 MCP server（太复杂）→ 改用 CLI only
2. snapshot JSON 大于 100KB（fixture 级别）→ 检查是否包含完整源码片段
3. smoke 测试失败 → 修复后重跑
4. `cargo fmt --check` 失败 → 格式化后重新检查

---

## 三、Verification Plan

必须全部通过：

```bash
cargo fmt --check                    # ✅ Stage 7
git diff --check                      # ✅ Stage 7
cargo test --test mcp_server          # ✅ Stage 7 (114 pass)
bash scripts/codelattice-mcp.sh --self-test  # ✅ Stage 7 (37 tools)
bash scripts/mcp-dogfood.sh           # ✅ Stage 7 (37 PASS)
bash scripts/webui-snapshot-smoke.sh  # ✅ Stage 7 (17 checks)
python3 -m json.tool fixtures/webui-snapshots/rust-portable-smoke.snapshot.json     # ✅ Stage 7
python3 -m json.tool fixtures/webui-snapshots/typescript-portable-smoke.snapshot.json # ✅ Stage 7
```

---

## 四、Risk Assessment

| Risk | Level | Mitigation |
|------|-------|------------|
| CLI JSON format 变更导致解析失败 | Low | 脚本做 graceful fallback |
| Python 3 不可用 | Very Low | macOS/Linux 默认有 |
| Fixture 路径含绝对路径泄露到 snapshot | Low | 脚本不替换路径；snapshot 中 root 保持原始值 |
| Smoke 在 CI 环境超时 | Low | CLI analyze 对 fixture < 2s |
