# WebUI Readiness / Snapshot Contract Pack — Closure Review

> **日期：** 2026-05-16
> **Preflight:** [2026-05-16-webui-readiness-preflight.md](2026-05-16-webui-readiness-preflight.md)
> **状态：** ✅ Closure complete — 所有 stage 通过

---

## 一、交付物清单

### 1.1 文档 (docs/webui/)

| 文件 | 行数 | 说明 |
|------|------|------|
| `docs/webui/README.md` | ~200 | 定位、5 视图规划、MCP vs WebUI 关系、信息架构原则 |
| `docs/webui/webui-mvp.md` | ~280 | Dashboard/Explore/Impact/Cleanup/Release 详细 layout + caution 渲染规范 |
| `docs/webui/webui-snapshot-contract.md` | ~550 | CodeLatticeWebSnapshotV1 完整 schema、字段稳定性、最小/完整样例 |

### 1.2 脚本

| 脚本 | 行数 | 功能 |
|------|------|------|
| `scripts/webui-snapshot.sh` | ~230 | 从 CLI analyze+quality 聚合为 V1 snapshot JSON |
| `scripts/webui-snapshot-smoke.sh` | ~180 | 自动生成 Rust/TS snapshot 并验证 17 项 check |

### 1.3 Fixtures

| 文件 | 大小 | 语言 |
|------|------|------|
| `fixtures/webui-snapshots/rust-portable-smoke.snapshot.json` | 5,266 B | rust |
| `fixtures/webui-snapshots/typescript-portable-smoke.snapshot.json` | 4,719 B | typescript |

### 1.4 更新的现有文件

| 文件 | 变更 |
|------|------|
| `README.md` | 新增 "WebUI Readiness" 小节（~40 行） |
| `CHANGELOG.md` | 新增 [Unreleased] Added 条目（WebUI readiness 全套） |
| `docs/plans/README.md` | 更新最后更新日期 + 增加 WebUI pack 索引 |

---

## 二、验证结果

### Stage 0 — Truth Gate

| Check | Result |
|-------|--------|
| Repo path = `/Users/jiangxuanyang/Desktop/codelattice` | ✅ |
| HEAD = `c8e98100c319778c2fb02c03a19d5e23c72c7952` | ✅ |
| Branch = `master` | ✅ |
| git status clean（仅 `.claude/` untracked） | ✅ |
| `cargo fmt --check` | ✅ PASS (clean) |
| `git diff --check` | ✅ PASS (clean) |
| `cargo test --test mcp_server` | ✅ 114 passed, 0 failed |
| `codelattice-mcp.sh --self-test` | ✅ 37 tools, all checks pass |
| `mcp-dogfood.sh` | ✅ 37 PASS, 0 FAIL |

### Stage 5 — Smoke Test

```
Prerequisites:     3/3 PASS (binary, python3)
Rust snapshot:     1/1 PASS (5266 bytes)
TypeScript snapshot: 1/1 PASS (4719 bytes)
Validate Rust:      7/7 PASS (JSON, schemaVersion, staticAnalysis, runtimeVerified, sourceFileCount, quality, limitations)
Validate TS:        7/7 PASS (同上)
─────────────────────────────
Total:              18/18 PASS, 0 FAILED
SMOKE PASSED
```

---

## 三、Contract 合规性

### 3.1 必填字段覆盖

| Section | 状态 | 来源工具 |
|---------|------|----------|
| `schemaVersion` = `"webui.snapshot.v1"` | ✅ | 脚本硬编码 |
| `generatedAt` (ISO 8601) | ✅ | `date` command |
| `generatedFrom.staticAnalysis == true` | ✅ | 脚本硬编码 |
| `generatedFrom.runtimeVerified == false` | ✅ | 脚本硬编码（永远 false） |
| `summary.sourceFileCount > 0` | ✅ | CLI analyze output |
| `quality` section 存在 | ✅ | CLI quality output |
| `limitations` section 存在（≥10 项） | ✅ | 脚本硬编码 stop-lines |

### 3.2 字段稳定性标注

| Section | 标注 | 符合 |
|---------|------|------|
| summary | stable | ✅ |
| quality | stable | ✅ |
| insights | heuristic | ✅ (`not_collected`) |
| explore | stable | ✅ (`not_collected`) |
| impact | stable | ✅ (`not_collected`) |
| cleanup.* | heuristic | ✅ (`not_collected`) |
| releaseReview.* | heuristic | ✅ (`not_collected`) |
| workflowPresets | stable | ✅ (`not_collected`) |
| limitations | stable | ✅ (11 items) |

---

## 四、边界合规性

| 边界 | 结果 |
|------|------|
| 只改 CodeLattice repo | ✅ 未触碰其他仓库 |
| 不新增前端框架 | ✅ 无 HTML/CSS/JS/TS/Vue/Svelte |
| 不引入包管理 | ✅ 无 npm/pnpm/yarn/node_modules |
| 不改 MCP 字段语义 | ✅ mcp_server.rs 未修改 |
| 不运行 promote | ✅ 未执行 promote-to-local-tool.sh |
| 不改真实项目源码 | ✅ 仅用 fixtures |
| 不声称 runtime proof | ✅ generatedFrom 全部 false |

---

## 五、已知限制

1. **insights/explore/impact/cleanup/releaseReview sections 默认 `not_collected`**：这些 section 的完整数据需要 MCP 工具聚合（如 project_insights, dead_code_candidates 等），CLI alone 无法提供。这是 design decision，不是 bug。
   - Future path：WebUI 可通过 on-demand MCP call 填充这些 section。
2. **snapshot 含绝对路径**：`root` 字段包含分析时的绝对路径。对于 portable fixture snapshots 这不影响可移植性（fixture 路径在 repo 内）。未来可增加 `--relative-path` 选项。
3. **Python 依赖**：脚本使用 `python3 -c` 做 JSON 聚合。macOS/Linux 均默认安装。Windows 需单独确认。
4. **topNodeKinds/topEdgeKinds 可能为空数组**：CLI analyze compact 模式下可能不含这些字段。不影响 contract 结构。

---

## 六、下一步建议（非本轮）

1. **MCP aggregation mode**: 让 webui-snapshot.sh 可选调用 MCP server 获取 insights/explore/cleanup 数据
2. **Relative root option**: `--relative-path` 使 root 字段使用 repo-relative 路径
3. **Snapshot diff tool**: 比较两个 snapshot 的字段级差异
4. **Tauri/Electron shell**: 当数据层就绪后，实现实际的前端渲染
