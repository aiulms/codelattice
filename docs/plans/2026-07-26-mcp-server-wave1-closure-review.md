# mcp_server.rs Wave 1 closure review

日期：2026-07-26
类型：wave closure review（多刀路线图 Wave 1 收尾）
来源路线图：[2026-07-26-mcp-server-multi-slice-roadmap-preflight.md](2026-07-26-mcp-server-multi-slice-roadmap-preflight.md)

## 1. Wave 1 目标与达成

**目标**：抽出 5 个高独立性纯函数簇，验证 mcp_server.rs 多刀拆分 playbook 在 cli crate 可行（兄弟 `mod` + `pub(crate)` + glob import），为 Wave 2-4 建立基础。

**达成**：✅ 全部 5 簇已抽出，playbook 验证通过。

## 2. 落地清单

| 刀 | Cluster | 新模块 | 行数 | commit |
|----|---------|--------|------|--------|
| 1 | C9 JSON output/error helpers | `mcp_json_helpers.rs` | 172 | `7514d1a` |
| 2 | C5 diagnose terms + C6 node helpers | `mcp_diagnose_terms.rs` (88) + `mcp_node_helpers.rs` (55) | 143 | `419ad00` |
| 3 | C3 decision guidance + C4 risk scoring | `mcp_decision_guidance.rs` (178) + `mcp_risk_scoring.rs` (216) | 394 | `d8621da` |

**Wave 1 合计抽出**：5 模块、709 行、24 个函数。

## 3. 文件变化

| 文件 | 前 | 后 | 变化 |
|------|----|----|------|
| `mcp_server.rs` | 32852 | 32240 | -612 (-1.9%) |
| 5 个新模块 | 0 | 709 | +709 |
| `lib.rs` | 29 | 34 | +5（5 个 mod 声明） |

## 4. Playbook 验证结果

| 验证项 | 结果 |
|--------|------|
| 兄弟私有 `mod` + `pub(crate)` + glob import 模式可行 | ✅ 与 mcp_facade/mcp_job 先例一致 |
| 入度极高的纯函数簇可平滑迁移（C9 合计 258 处调用） | ✅ glob import 后零调用点改动 |
| 强内聚簇整体迁移（C3 decision_guidance、C4 enrich_risk_item） | ✅ 内部调用随迁移自动经 glob 解析 |
| 行为等价可证（test + smoke 字节级） | ✅ 每刀 738/738 pass + sha256 一致 |
| 大块行删除的 precommit critical 为结构性误报 | ✅ 见各刀 closure（个体 risk 均 LOW） |

**关键经验**：
- 合并相邻小簇成一刀可减少 build/test 周期（C5+C6、C3+C4 各一刀）
- 运输笔误（如 `as_array` → `array`）由编译器立即捕获，证明"先 build 再 test"顺序必要
- cargo fmt 会重排 `use` 和 `mod` 字母序，提交前必须 `cargo fmt` 一次

## 5. 统一 Invariants（每刀均守住）

| Invariant | 状态 |
|-----------|------|
| `cargo test --workspace` 738/738 pass（全程零漂移） | ✅ |
| CLI smoke c1-same-module sha256 字节级一致 | ✅ |
| `cargo fmt --check` + `git diff --check` clean | ✅ |
| MCP tool contract 不变 | ✅ |
| 4 个 pub 项签名不变（run_mcp_server / WarmCacheMeta / WarmTrace / Freshness） | ✅ |
| 不动 GraphView / McpCache 等枢纽 | ✅ |
| 不把 `pub(crate)` 升为 `pub` | ✅ |

## 6. 路线图进度更新

| Wave | 目标抽出 | 已抽出 | 状态 | mcp_server.rs |
|------|----------|--------|------|---------------|
| **1** | ~518 | **612** | ✅ 完成（超目标） | 32240 |
| 2 | ~2331 | 0 | pending | — |
| 3 | ~4111 | 0 | pending（评估点） | — |
| 4 | 远景 | 0 | 不在近期范围 | — |

Wave 1 实际抽出 612 行，略超预估 518（C3/C4 实测比预估大）。

## 7. 下一步决策点

按路线图 §5.2，Wave 1 完成后进入 Wave 2（数据/配置子域，~2331 行）。但 Wave 1 的收益（-1.9%）印证了路线图的核心判断：**单刀收益有限，要显著瘦身需持续多刀**。

建议在启动 Wave 2 前确认：继续 Wave 2（C38 workflow presets / C26 tools_list / C37 automation parsers / C1 path safety / C13 staleness），还是暂停转向其他价值更高的工作（CALLS resolution rate / text fallback 第三刀）。Wave 2 各簇独立性同样高，playbook 已验证，可随时续做。

## 8. 不能自动重开的线

- 不把 5 个新模块内容 merge 回 mcp_server.rs
- 不把 `pub(crate)` 升为 `pub`
- 不改 24 个迁出函数的签名
- Wave 2 前不新增依赖 GraphView/McpCache 签名的函数到这 5 个模块（它们必须保持枢纽无关）
