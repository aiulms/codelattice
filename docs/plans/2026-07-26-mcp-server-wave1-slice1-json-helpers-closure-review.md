# mcp_server.rs Wave 1 Slice 1 — mcp_json_helpers extraction closure review

日期：2026-07-26
类型：implementation closure review
来源路线图：[2026-07-26-mcp-server-multi-slice-roadmap-preflight.md](2026-07-26-mcp-server-multi-slice-roadmap-preflight.md)（Wave 1，序 1.5 C9）

## 1. Landed Reality

### 1.1 File layout 变化

| 文件 | 操作 | 前行数 | 新行数 | 变化 |
|------|------|--------|--------|------|
| `mcp_server.rs` | 修改 | 32852 | 32698 | -154 (-0.5%) |
| `mcp_json_helpers.rs` | 新增 | — | 172 | +172 |
| `lib.rs` | 修改 | 28 | 29 | +1 |

### 1.2 提取内容

从 `mcp_server.rs` 原 lines 1334-1491 提取 9 个纯函数到 `mcp_json_helpers.rs`：

| 符号 | 新可见性 | 原行号 | 入度 |
|------|----------|--------|------|
| `mcp_error` | `pub(crate)` | 1339 | 107 |
| `mcp_error_detail` | `pub(crate)` | 1346 | 4 |
| `mcp_error_with_hint` | `pub(crate)` | 1354 | 12 |
| `tool_error` | `pub(crate)` | 1364 | 1 |
| `tool_result` | `pub(crate)` | 1371 | 80 |
| `tool_result_cached` | `pub(crate)` | 1379 | 1 |
| `inject_cache_meta` | `pub(crate)` | 1386 | 2 |
| `merge_cache_and_result` | `pub(crate)` | 1400 | 39 |
| `read_source_snippet` | `pub(crate)` | 1414 | 12 |

### 1.3 结构调整

迁移区原含两个 banner：
- `// Error helpers`（原 L1334）—— 随函数迁走
- `// Two-Layer Analysis Cache (v0.3 memory + v0.8 persistent)`（原 L1395）—— 此 banner 原本覆盖 `merge_cache_and_result` / `read_source_snippet`（迁出）和 `CacheKey` / `CacheEntry`（保留）。迁出 output helpers 后，在 `CacheKey` 前补回该 banner，保持 cache 层结构标识。

### 1.4 验证 playbook 可行性

本刀是 mcp_server.rs 多刀拆分的"stdlib_tables 时刻"——验证了：
- 兄弟 `mod`（私有）+ `pub(crate)` + glob import 模式在 cli crate 可行（与 mcp_facade/mcp_job 先例一致）
- 入度极高的纯函数簇（C9 合计 258 处调用）通过 `use crate::mcp_json_helpers::*` 平滑迁移，零调用点改动
- cargo test + CLI smoke 字节级一致，行为等价成立

## 2. Invariants

| Invariant | 状态 |
|-----------|------|
| `cargo test --workspace` 通过数与基线一致 | ✅ 738/738 pass（基线 738/738） |
| CLI smoke 输出字节级一致 | ✅ c1-same-module sha256 before/after 完全相同 |
| `cargo fmt --check` clean | ✅ |
| `git diff --check` clean | ✅ |
| MCP tool contract 不变 | ✅ |
| 4 个 pub 项签名不变 | ✅ |

## 3. 验证结果

```bash
cargo build --workspace    # Finished, 零 error
cargo fmt --check          # PASS（cargo fmt 修正 lib.rs mod 字母序：mcp_json_helpers 排在 mcp_job 之后）
cargo test --workspace     # 738 passed, 0 failed
git diff --check           # PASS
CLI smoke c1               # sha256 6efe3204... before/after 字节一致
```

## 4. Stop-Line

| Stop-Line | 守住 |
|-----------|------|
| 不改 MCP tool contract / 4 pub 项签名 | ✅ |
| 不改 runtime 语义 | ✅ 纯搬运 |
| 不动 GraphView / McpCache / 其他枢纽 | ✅ |
| 不动 fixtures / Cargo.toml | ✅ |
| 不把 `pub(crate)` 升为 `pub` | ✅ |
| 仅改 mcp_server.rs / mcp_json_helpers.rs / lib.rs | ✅ |

## 5. Residual Risk

| 风险 | 级别 | 说明 |
|------|------|------|
| 减量仅 154 行 | LOW | 符合预期，Wave 1 单刀目标就是验证 playbook 而非大幅瘦身 |
| `read_source_snippet` 读磁盘 | LOW | 无枢纽依赖，由全量 test 覆盖 |
| mod 字母序 `mcp_job` < `mcp_json_helpers` | LOW | rustfmt 自动修正，`jo` < `js` |

## 6. 路线图进度

| Wave | 序 | Cluster | 状态 | mcp_server.rs 行数 |
|------|----|---------|------|---------------------|
| 1 | 1.5 | C9 JSON helpers | ✅ 本刀 | 32698 |
| 1 | 1.1 | C5 diagnose terms | pending | — |
| 1 | 1.2 | C6 node helpers | pending | — |
| 1 | 1.3 | C4 risk scoring | pending | — |
| 1 | 1.4 | C3 decision guidance | pending | — |

Wave 1 目标 ~518 行抽出，本刀完成 154 行（30%）。剩余 4 簇预计 ~364 行。

## 7. 不能自动重开的线

- 不把 mcp_json_helpers.rs 内容 merge 回 mcp_server.rs
- 不把 `pub(crate)` 升为 `pub`
- 不改 9 个函数的签名（入度 258，改动面巨大）
