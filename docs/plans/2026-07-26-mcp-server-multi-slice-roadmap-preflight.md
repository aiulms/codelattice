# mcp_server.rs 多刀拆分路线图 Preflight

> **日期：** 2026-07-26
> **状态：** Preflight / 路线图初稿（docs-only，不改代码）
> **目的：** 为 `crates/cli/src/mcp_server.rs`（32852 行、441 符号、37 version-banner 段）制定多刀行为等价拆分路线，明确每一刀的独立性评级、抽取顺序、枢纽类型处理策略与触发条件。
> **Stop-line：** 本文不启动代码改动、不改 runtime 语义、不改 MCP tool contract、不改公共 API。

---

## 一、为什么需要这份路线图

`mcp_server.rs` 已达 **32852 行**，是当前 MCP/AI sidecar 主线的唯一载体。它与 `calls.rs` 性质完全不同：

| 维度 | calls.rs（第二刀已落地） | mcp_server.rs |
|------|--------------------------|---------------|
| 规模 | 2364 行 | **32852 行** |
| 符号数 | ~30 | **441** |
| 结构 | 7 个清晰 section | **37 个 version-banner 段** |
| 枢纽类型 | 无（查询索引可独立） | **2 个全局枢纽：GraphView(76 ref) / McpCache(78 ref)** |
| 单刀可抽量 | 378 行（16%） | **≤600 行/刀（≤2%）** |

**核心结论：mcp_server.rs 不存在"一刀大幅瘦身"的可能。** 任何单刀只能抽出几百行。要把文件降到健康水位（目标 ≤ 8000 行），需要 **8-12 刀连续提取**，且必须按"先叶后枢纽"的严格顺序，否则会触发枢纽类型的级联改动。

这份路线图的作用是把多刀顺序、依赖关系、触发条件提前冻结，避免每一刀都重新评估。

---

## 二、文件结构总览（量化勘察结果）

### 2.1 全局枢纽（动不得，至少前期）

| 枢纽 | 行范围 | 引用数 | 持有 | 处理策略 |
|------|--------|--------|------|----------|
| **`McpCache`** (struct + impl) | 3001-4749 (~1740 行) | **78** | CacheEntry map、ConversationContext、persistent cache 桥、调 `mcp_job::MCP_JOBS.submit` | **全程留在 mcp_server.rs**。每个 `handle_*` 签名都带 `&mut McpCache`，是 dispatch 的中枢状态。 |
| **`GraphView`** (struct + impl) | 5825-6917 (~1090 行) | **76** | nodes_by_id、symbols_by_name、outgoing/incoming、diagnostics、`Arc<DocScanner>` | **全程留在 mcp_server.rs**。70+ handler 通过 `&GraphView` 查图；CacheEntry own 一个。持有 DocScanner 造成 C21 与它强耦合。 |

**含义：** 凡是签名带 `&GraphView` 或 `&mut McpCache` 的函数/cluster，都不能在前期独立抽出——必须等枢纽本身被处理（最后阶段）或通过 trait 抽象解耦（高风险，暂不考虑）。

### 2.2 公共 API 契约（必须保持可达）

mcp_server.rs 仅 4 个 `pub` 项，其余 ~437 项私有：

- `pub enum Freshness` (L145)
- `pub struct WarmCacheMeta` (L1648)
- `pub struct WarmTrace` (L1659)
- `pub fn run_mcp_server()` (L30118)

外部消费点：
- `lib.rs:5604` → `mcp_server::run_mcp_server()`
- `mcp_job.rs:58, :80` → `crate::mcp_server::WarmCacheMeta`

**约束：** 任何抽取必须保证这 4 项仍可经 `crate::mcp_server::...` 访问（通过 mcp_server.rs 内的 `pub use crate::<submodule>::...` re-export 即可）。

### 2.3 已有拆分先例

`crates/cli/src/` 已有兄弟模块 `mcp_facade.rs`、`mcp_job.rs`、`ai_runtime.rs`，均在 lib.rs 用**私有 `mod`** 声明（非 `pub mod`），内部项用 `pub(crate)` 暴露。这是已验证的拆分模式，本路线图所有新子模块沿用此模式。

---

## 三、Cluster 独立性评级与抽取顺序

基于入度（被调用次数）、是否依赖枢纽类型、内部聚合度，把 37 个 cluster 分成 4 个波次。

> **评级标准：**
> - 🟢 **Tier-1 易抽**：纯函数，不依赖 GraphView/McpCache 类型，入度低或可 glob-import 平滑迁移。
> - 🟡 **Tier-2 可抽**：依赖 `&Value`（携带图数据但函数只读字段）或少量兄弟 helper，需小心可见性。
> - 🟠 **Tier-3 难抽**：签名带 `&GraphView` 或 `&mut McpCache`，必须连枢纽一起考虑。
> - 🔴 **Tier-4 枢纽**：GraphView / McpCache 本体，最后处理。

### Wave 1 — 纯函数叶子（最高独立性，建立 playbook）

| 序 | Cluster | 行范围 | 行数 | 入度 | 目标模块 | 评级 |
|----|---------|--------|------|------|----------|------|
| 1.1 | C5 diagnose term helpers | 920-993 | ~74 | 8 | `mcp_diagnose_terms.rs` | 🟢 |
| 1.2 | C6 node display helpers | 995-1033 | ~39 | 54 | `mcp_node_helpers.rs` | 🟢 |
| 1.3 | C4 risk scoring / calibration | 735-918 | ~184 | 26 | `mcp_risk_scoring.rs` | 🟢 |
| 1.4 | C3 decision guidance / mode semantics | 574-733 | ~160 | 中 | `mcp_decision_guidance.rs` | 🟢 |
| 1.5 | C9 JSON output/error helpers | 1334-1394 | ~61 | **226** | `mcp_json_helpers.rs` | 🟢（注意回归面） |

**Wave 1 合计：** ~518 行抽出，mcp_server.rs 32852 → ~32334。

**关键说明：**
- C9 入度极高（`mcp_error` 107、`tool_result` 80、`merge_cache_and_result` 39），但函数本身纯，glob-import 后调用点无需改动，回归风险由全量 `cargo test`（24381 行测试）覆盖。**建议 C9 作为 Wave 1 的第一刀**——它有独立 banner、纯函数、被最多处调用，抽完后 mcp_server.rs 顶部加一行 `use crate::mcp_json_helpers::*`，验证最简单也最能证明 playbook 可行。
- `read_source_snippet` 读磁盘，但无枢纽依赖，随 C9 一起走。

### Wave 2 — 数据/配置子域（自包含 subdomain）

| 序 | Cluster | 行范围 | 行数 | 入度 | 目标模块 | 评级 |
|----|---------|--------|------|------|----------|------|
| 2.1 | C38 AI Workflow preset builders（纯数据） | 22673-22918 | ~246 | 低 | `mcp_workflow_presets.rs` | 🟢 |
| 2.2 | C26 tools_list JSON catalog + toolset gate | 13028-14108 | ~1081 | 低 | `mcp_tools_catalog.rs` | 🟡（`McpToolset` 可能被 dispatch 引用） |
| 2.3 | C37 automation parsers 子集（parse_ci/makefile/dockerfile/shell，**不含** handle_automation_graph） | 21673-22672 内 | ~600 | 各 2 | `mcp_automation_parsers.rs` | 🟡 |
| 2.4 | C1 path validation / safety | 154-288 | ~135 | 37 | `mcp_path_safety.rs` | 🟢 |
| 2.5 | C13 staleness / fingerprint | 2063-2331 | ~269 | 低 | `mcp_staleness.rs` | 🟢 |

**Wave 2 合计：** ~2331 行抽出，累计 mcp_server.rs ~32334 → ~30003。

**关键说明：**
- C37 的 `handle_automation_graph` 签名带 `&mut McpCache`（实测引用枢纽 4 处），**留在 mcp_server.rs**；只抽 parse_* 纯解析函数。这修正了初步勘察中"C37 highly self-contained"的判断。
- C26 的 `tools_list` 是 860 行静态 JSON，极易抽；但 `McpToolset` enum 被 dispatch 的 toolset-gate 引用，需验证是否要随 enum 一起迁移或保留。

### Wave 3 — 图算法簇（依赖 &GraphView 但不改它）

| 序 | Cluster | 行范围 | 行数 | 评级 | 说明 |
|----|---------|--------|------|------|------|
| 3.1 | C27 dead code / reachability classifiers | 14109-14810 | ~702 | 🟠 | 纯算法 over `&GraphView`，handler 留原处 |
| 3.2 | C28 impact / hotspot / drift 算法 | 14811-16622 | ~1812 | 🟠 | 同上，`handle_*` 留原处，只抽 compute_* |
| 3.3 | C32 external API surface 算法 | 18919-19475 | ~557 | 🟠 | 同上 |
| 3.4 | C34 breaking-change review | 20154-20760 | ~607 | 🟡 | 用 C6 node helpers（已抽出） |
| 3.5 | C30 entry point / reachability map 算法 | 17827-18259 | ~433 | 🟠 | 纯算法 |

**Wave 3 策略：** 这些 cluster 的 **handler（`handle_*`）留在 mcp_server.rs**（因签名带 `&mut McpCache`），只把 **compute/detect/classify 纯算法函数**抽到子模块，handler 通过 `use crate::<sub>::compute_*` 调用。算法函数签名带 `&GraphView`，需把 GraphView 提升为 `pub(crate)` 或在子模块 `use super::GraphView`——**这是 Wave 3 的关键可见性决策**。

**Wave 3 合计：** ~4111 行（仅算法部分）抽出，累计 ~30003 → ~25892。

### Wave 4 — 枢纽处理（最后，最高风险）

| 序 | Cluster | 行范围 | 行数 | 评级 |
|----|---------|--------|------|------|
| 4.1 | C21 DocScanner（被 GraphView 持有 Arc） | 7581-9511 | ~1931 | 🟠→🔴 |
| 4.2 | C15 GraphView（枢纽本体） | 5825-6917 | ~1093 | 🔴 |
| 4.3 | C16 McpCache（枢纽本体） | 3001-4749 | ~1749 | 🔴 |
| 4.4 | C40 Facade Tools / workflow engine | 23176-30117 | ~6942 | 🔴 |
| 4.5 | C41 run_mcp_server + ask engine | 30118-EOF | ~2734 | 🔴 |

**Wave 4 不在本路线图的执行范围内**，仅作远景记录。处理枢纽需要：
- 把 GraphView 迁到 `mcp_graph_view.rs` 并提升为 `pub(crate)`，所有 handler 子模块 `use crate::mcp_graph_view::GraphView`。
- 把 McpCache 迁到 `mcp_cache.rs`，同样 `pub(crate)`。
- C40 facade engine（6942 行，最大单段）需先抽 Wave 1-3 减少内部依赖后再评估，可能需要按 workflow 子状态机进一步切分。

**Wave 4 目标：** 最终 mcp_server.rs 降到 ≤ 8000 行（仅保留 dispatch + 枢纽 + 无法归类的 handler）。

---

## 四、每一刀的统一 playbook（复刻 calls_index / stdlib_tables 纪律）

每一刀遵循已验证的 6 步：

1. **基线**：`cargo test --workspace` 记 pass count + CLI smoke 保存 before JSON。
2. **建子模块**：从 mcp_server.rs 原样搬运；加 `//!` header 注明来源（原行号、日期、行为等价提取）；可见性 `fn`/`struct`/字段统一 `pub(crate)`；保留所有 doc/banner。
3. **改 mcp_server.rs**：删除迁移段；顶部加 `use crate::<submodule>::*;`。
4. **改 lib.rs**：加 `mod <submodule>;`（私有 mod，对齐 mcp_facade/mcp_job 先例）。
5. **验证全绿**：`cargo fmt --check` + `cargo test`（pass count 一致）+ `git diff --check` + CLI smoke（字节一致）+ `scripts/codelattice-precommit-check.sh`。
6. **closure review** + 更新本路线图的进度表 + 更新 AGENTS.md（若新增 quality-watch 条目）。

**Write Set 模板（每刀 ≤ 3 文件）：**
- `crates/cli/src/<new_module>.rs`（新建）
- `crates/cli/src/mcp_server.rs`（删 + 加 use）
- `crates/cli/src/lib.rs`（加 mod）
- 可选：`docs/plans/<date>-mcp-<slice>-closure-review.md`

**Forbidden Set（每刀通用）：**
- 不改 MCP tool contract（tool name / 参数 / 返回 shape）
- 不改 4 个 pub 项签名（run_mcp_server / WarmCacheMeta / WarmTrace / Freshness）
- 不改 runtime 语义、confidence/reason、stop-line
- 不动 fixtures / expected
- 不动 Cargo.toml
- 不在 Wave 4 前动 GraphView / McpCache 本体
- 不把 `pub(crate)` 升为 `pub`

**Stop Line（每刀通用）：**
- `cargo test` 任何已有用例由 pass 转 fail 且非搬运笔误
- CLI smoke 输出漂移
- 发现未识别枢纽耦合需超 write set
- precommit 报 high/critical **且** changedSymbol 个体 risk 非 LOW（注：大块行删除会触发结构性 critical 误报，见 calls_index closure §7，需逐项核查）

---

## 五、触发条件与节奏控制

### 5.1 何时启动下一刀

满足任一即触发：
- 上一刀已 commit + push 到 gitcode master
- `cargo test` 全绿且与基线一致
- 本路线图对应 Wave 还有剩余 cluster

### 5.2 何时暂停拆分

满足任一即暂停，转为其他工作：
- mcp_server.rs 降到 ≤ Wave 3 目标（~25892 行）后，**收益递减**：剩余都是枢纽/引擎，每刀风险骤升。建议此时评估是否值得继续，还是把精力转向 CALLS resolution rate 等功能价值更高的方向。
- 单刀 precommit 出现**非误报**的 high/critical（changedSymbol 个体 risk = HIGH/Critical）。
- 出现新功能需求需要修改 mcp_server.rs，拆分让位。

### 5.3 何时重开枢纽处理（Wave 4）

仅在以下情况考虑：
- mcp_server.rs 因新增功能持续膨胀，Wave 1-3 抽出后被新代码抵消
- GraphView / McpCache 本身需要重构（例如引入 trait 支持多 backend）
- 有明确的性能/可测试性需求要求把枢纽类型单独编译

---

## 六、风险与已知陷阱

| 风险 | 级别 | 缓解 |
|------|------|------|
| 大块行删除触发 precommit critical 误报 | LOW | calls_index closure §7 已验证此模式；逐项核查 changedSymbol 个体 risk |
| C9 入度 226 处，glob-import 遗漏导致编译错误 | LOW | 编译器会立即报错；`cargo build` 是第一道关 |
| Wave 3 算法函数带 `&GraphView`，可见性决策错误 | MEDIUM | Wave 3 第一刀前先做小 preflight：确认 GraphView 提升为 pub(crate) 的影响面 |
| 抽取后 `use` 冲突（同名符号） | LOW | mcp_server.rs 顶部已有 `use crate::mcp_facade::...`；新模块用具名 import 避免冲突 |
| 枢纽处理（Wave 4）触发级联 | HIGH | 本路线图明确 Wave 4 不在近期范围；触发前需单独 preflight |
| `read_source_snippet`（C9）读磁盘，行为非纯 | LOW | 随 C9 迁移，由测试覆盖；无枢纽依赖 |

---

## 七、成功指标

| 阶段 | mcp_server.rs 行数 | 累计抽出 | 状态 |
|------|---------------------|----------|------|
| 起点（2026-07-26） | 32852 | 0 | — |
| Wave 1 完成（2026-07-26） | **32240** | **612** | ✅ 已完成（commit `7514d1a`/`419ad00`/`d8621da`） |
| Wave 2 完成 | ~30003 | ~2849 | 目标：数据子域清理 |
| Wave 3 完成 | ~25892 | ~6960 | 目标：图算法解耦，**建议评估点** |
| Wave 4 完成（远景） | ≤ 8000 | ~24852 | 目标：枢纽迁出，需单独决策 |

> Wave 1 实际抽出 612 行（5 模块），略超预估 518。详见 [Wave 1 closure review](2026-07-26-mcp-server-wave1-closure-review.md)。

**关键评估点在 Wave 3 结束时**：届时 mcp_server.rs 约 25892 行，剩余都是枢纽/引擎。应停下来判断：继续 Wave 4（高风险高投入）是否值得，还是转向功能价值更高的工作。

---

## 八、与现有治理的关系

- **本路线图不创建新的 stop-line**，mcp_server.rs 拆分不是 AGENTS.md 当前的强制 quality-watch（calls.rs 才是）。
- 建议在 Wave 1 第一刀落地后，根据实际经验决定是否把 mcp_server.rs 纳入 AGENTS.md 的 quality-watch（参照 calls.rs 的格式）。
- 本路线图遵循 AGENTS.md 的执行序列：preflight（本文）→ 每刀 execution card（可选，小刀可省）→ implementation → closure review。

---

## 九、Raw notes（勘察依据）

- 结构勘察：`mcp_server.rs` 441 top-level items，37 version-banner 段（`// ====...`）
- 枢纽引用计数：GraphView 76、McpCache 78（grep 实测）
- Wave 1 候选入度：C5=8、C6=54、C4=26、C9=226（grep 实测）
- C37 实测引用枢纽 4 处（handle_automation_graph 带 `&mut McpCache`），修正初步勘察
- GraphView 持有 `Arc<DocScanner>`（L6148），造成 C21-C15 强耦合
- 测试基线：`crates/cli/tests/mcp_server.rs` 24381 行（黑盒回归网）
- 已有兄弟模块先例：mcp_facade.rs、mcp_job.rs、ai_runtime.rs（均私有 mod + pub(crate)）

本文件为路线图 preflight，不启动任何代码改动。Wave 1 第一刀的具体执行需另起 execution card 或直接按 §四 playbook 推进。
