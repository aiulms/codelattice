# CALLS calls_index extraction — closure review

日期：2026-07-26
类型：implementation closure review
来源计划：本仓对话 preflight（calls.rs 第二刀，抽 3 个 Call Index）
前序提取：[2026-05-04 stdlib_tables extraction](2026-05-04-calls-stdlib-tables-extraction-closure-review.md)

## 1. Landed Reality

### 1.1 File layout 变化

| 文件 | 操作 | 前行数 | 新行数 | 变化 |
|------|------|--------|--------|------|
| `calls.rs` | 修改 | 2364 | 1984 | -380 (-16.1%) |
| `calls_index.rs` | 新增 | — | 466 | +466 |
| `lib.rs` | 修改 | 21 | 22 | +1 |

### 1.2 提取内容

从 `calls.rs` 原 lines 190-567 整体提取 3 个查询索引及其 builder / impl 到 `calls_index.rs`：

| 符号 | 新可见性 | 原行号 |
|------|----------|--------|
| `CalleeMatch` (struct + 字段) | `pub(crate)` | 194–205 |
| `CalleeIndex` (struct) | `pub(crate)` | 207–219 |
| `build_callee_index` | `pub(crate)` | 221–300 |
| `impl CalleeIndex`（6 原方法 + 2 新封装入口） | 方法 `pub(crate)` | 302–381 |
| `ImportBinding` (struct + 字段) | `pub(crate)` | 387–399 |
| `ImportBindingTable` (struct) | `pub(crate)` | 401–403 |
| `build_import_binding_table` | `pub(crate)` | 405–431 |
| `build_wildcard_module_map` | `pub(crate)` | 449–469 |
| `impl ImportBindingTable`（2 原方法 + 1 新封装入口） | 方法 `pub(crate)` | 471–495 |
| `CallerInfo` (struct + 字段) | `pub(crate)` | 501–508 |
| `CallerIndex` (struct) | `pub(crate)` | 510–512 |
| `build_caller_index` | `pub(crate)` | 514–542 |
| `impl CallerIndex`（1 方法） | 方法 `pub(crate)` | 544–567 |

### 1.3 新增的 3 个封装入口（行为等价前提下最小改动）

为消除 calls.rs 对迁移字段/构造的直接访问，新增 3 个 `pub(crate)` 方法，**不引入新的语义逻辑**：

| 新增方法 | 位置 | 替代的原直接访问 |
|----------|------|------------------|
| `CalleeIndex::set_wildcard_modules(&mut self, map)` | calls_index.rs | calls.rs 原 L52 `symbol_index.wildcard_modules = ...`（字段写） |
| `CalleeIndex::wildcard_modules_for(&self, src) -> Option<&HashSet<String>>` | calls_index.rs | calls.rs 原 L1059 `symbol_index.wildcard_modules.get(...)`（字段读） |
| `ImportBindingTable::empty() -> Self` | calls_index.rs | calls.rs 原 L163-165 `ImportBindingTable { bindings: HashMap::new() }`（结构体字面量） |

字段 `CalleeIndex.wildcard_modules`、`ImportBindingTable.bindings`、`CallerIndex.by_file`、`by_module_and_name` 等全部保持模块内私有，不对外暴露。

保留在 `calls.rs`：
- 公共入口 `extract_and_resolve_calls` / `extract_delta_calls` / `CallExtractionResult`（签名不变）
- tree-sitter 提取段、resolve 逻辑段、text fallback 段（本轮不动）

### 1.4 未提取内容

按 preflight 决策，本轮**未提取**：
- Text fallback 子系统（~376 行）：与 resolve 逻辑段共享 `resolve_free_function` / `resolve_associated_function`，无法干净移动；需单独 slice 评估共享函数的可见性提升。留待第三刀。
- tree-sitter 提取段（~581 行）与 resolve 逻辑段（~837 行）：核心策略区，耦合更深，后续 slice。

## 2. Invariants

| Invariant | 状态 |
|-----------|------|
| `cargo test --workspace` 通过数与基线一致 | ✅ 738/738 pass（基线 738/738，零变化） |
| 23 个 call-resolution fixtures 零 golden drift | ✅ `call_comparison_passes_for_all_fixtures` pass |
| graph / import / symbol fixtures 零 golden drift | ✅ 全量 test pass |
| CLI smoke `--include calls --include graph` 输出字节级一致 | ✅ c1-same-module sha256 before/after 完全相同 |
| c14-wildcard-disambiguation 输出正常（覆盖新 getter 路径） | ✅ |
| `cargo fmt --check` clean | ✅ |
| `git diff --check` clean | ✅ |
| 不新增/删除/重排 fixture 文件 | ✅ |
| 公共 API 签名不变 | ✅ 3 项 pub 项签名零变化 |

## 3. 验证结果

```bash
# before 基线（建立快照）
cargo test --workspace            # 738 passed, 0 failed
CLI smoke c1                      # sha256 7bbdbbe2...（保存）

# after（提取后）
cargo build --workspace           # Finished, project-model 零 error/warning
cargo fmt --check                 # PASS（cargo fmt 修正 1 处 getter 签名换行）
cargo test --workspace            # 738 passed, 0 failed（与基线一致）
git diff --check                  # PASS
CLI smoke c1                      # sha256 7bbdbbe2...（与 before 字节一致）
CLI smoke c14-wildcard            # 正常产出（覆盖 wildcard_modules_for getter）
scripts/codelattice-precommit-check.sh  # exit 0
```

Test 明细与基线完全一致（738 用例无增减、无由 pass 转 fail）。

## 4. Public API / CLI / Graph Schema 影响

- **Public API**：不影响。所有提取符号为 `pub(crate)`；3 项 pub 项（`extract_and_resolve_calls` / `extract_delta_calls` / `CallExtractionResult`）签名不变。
- **CLI output**：不影响。c1 smoke 输出 sha256 字节级一致。
- **Graph schema**：不影响。
- **Expected fixtures**：不影响。零 golden drift。
- **GitNexus-RC adapter**：不影响。唯一集成缝 `output.rs:124 -> calls::extract_and_resolve_calls` 未动。

## 5. Comment Policy

本轮为纯移动重构，未新增语义边界。所有原 `///` doc 注释与中文语义注释（含 confidence/reason 选择理由、stop-line heuristic bridge 说明、wildcard 消歧策略注释）原样保留。calls_index.rs 顶部新增 `//!` module doc 说明来源（calls.rs 原 190-567，2026-07-26 行为等价提取）、三个索引的职责，以及"所有项 pub(crate)、不对外暴露"的可见性约定。

## 6. Stop-Line

| Stop-Line | 守住 |
|-----------|------|
| 不碰 model.rs / graph.rs / output.rs / 其他 runtime 文件 | ✅ 仅改 calls.rs / calls_index.rs / lib.rs |
| 不修改 resolution strategy 语义 | ✅ 纯搬运 |
| 不新增/删除/重排 fixture | ✅ |
| 不修改 expected-*.json | ✅ |
| 不做 text fallback / tree-sitter / resolve 提取（本轮范围外） | ✅ |
| 不混入新的 CALLS resolution 规则 | ✅ |
| 不新增依赖 | ✅ |
| 不新增 diagnostic code | ✅ |
| 不做 `pub` 导出（仅 `pub(crate)`） | ✅ |
| 3 项公共 API 签名不变 | ✅ |

## 7. Native Precommit Risk 评估

`scripts/codelattice-precommit-check.sh` exit 0，但 detect-changes 报 workspace-level `risk: critical`。经逐项核查判定为**误报，非语义风险**：

- changedSymbols：`extract_and_resolve_calls` / `extract_delta_calls` / `classify_callee` / `diagnostic` —— 均为 calls.rs 中**保留**的符号，detect-changes 因周围代码被大块删除而检测到位置/上下文变化，但实现与签名零变化。
- 每个 changedSymbol 的个体 `risk` 字段均为 **`LOW`**（最多 1 个 direct caller，即已知的 `extract_and_resolve_calls -> output.rs:124` 唯一缝）。
- `overallRisk: HIGH`（symbol 级）→ workspace 级 `critical`，主要由 2 个 csharp 误判边界 + 113 个低信号 fixture project 汇总驱动，与本次 Rust 提取无关。
- 行为等价证据链完整：738/738 test 一致 + CLI smoke 字节一致 + 个体符号全 LOW + 唯一集成缝未动。

结论：critical 为结构性误报，可安全继续。后续若 native detect-changes 对"大块行删除类纯重构"持续误报 critical，应作为 known limitation 记录。

## 8. Residual Risk

| 风险 | 级别 | 说明 |
|------|------|------|
| calls.rs 仍 1984 行 | LOW | 已从 2364 降至 1984，回到 2000 线下方。text fallback 提取后可进一步降至 ~1608。 |
| Text fallback 未提取 | LOW | ~376 行，与 resolve 逻辑共享 2 个函数，留待第三刀；当前不影响功能维护。 |
| 新增 3 个封装方法 | LOW | `set_wildcard_modules` / `wildcard_modules_for` / `empty` 为纯委托，无新逻辑；已由 c14 wildcard fixture + 全量 test 覆盖。 |
| `extract_delta_calls` 仍为 dead public API | LOW | 全仓零消费者；本轮保留其签名以避免 API 破坏，留待单独清理决策。 |

## 9. 下一次触发条件

当以下任一条件满足时，触发第三刀（text fallback 提取，calls_text.rs）：
- `calls.rs` 再次超过 2000 行
- 需要新增第 8 条 resolution strategy
- 需要新增新的 call kind
- 决定清理 `extract_delta_calls`（届时可连同其依赖的 text fallback 一起评估）

第三刀预评估：text fallback（~376 行）与 resolve 逻辑段共享 `resolve_free_function` / `resolve_associated_function`。提取前需决定其一：
- (a) 把这 2 个共享 resolve 函数提升为 `pub(crate)` 并留在 calls.rs，text fallback 迁移到 calls_text.rs 后 `use crate::calls::*`（但 calls.rs 内的私有函数无法跨模块引用，需提升可见性）；
- (b) 把共享 resolve 函数连同 text fallback 一起迁到 calls_text.rs，resolve 逻辑段的其余部分再单独评估。

## 10. 不能自动重开的线

- 不把已提取的 calls_index.rs 内容 merge 回 calls.rs
- 不在 text fallback 提取前新增 CALLS 策略
- 不把 `pub(crate)` 升为 `pub`
- 不移除新增的 3 个封装方法（它们是字段私有化的必要入口）
