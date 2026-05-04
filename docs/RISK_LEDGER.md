# GitNexus Rust-core 风险记录

最后更新：2026-05-04

## 用途

本文件记录 Rust-core 的已知风险、残留限制和防守规则。
完整风险记录（含 GitNexus-RC 侧风险）见 GitNexus-RC `docs/language-support/RISK_LEDGER.md`。

---

## 零、Active Bug Gate

### 0.2 Large source file quality watch

状态：**ACTIVE quality watch**

当前观察（2026-05-04）：

- `crates/project-model/src/calls.rs` 已从 2161 行拆分至 1858 行（2026-05-04 stdlib_tables 提取，-14.0%）。
- 新增 `stdlib_tables.rs`（311 行）：std lib tables + helpers 已独立为模块。
- 仍留 calls.rs：text fallback（~337 行）、CalleeIndex/ImportBindingTable/CallerIndex（~233 行）、resolution strategies。
- 下一次触发：calls.rs 再次超过 2000 行，或新增第 8 条 resolution strategy。

风险级别：**MEDIUM**（短期测试可覆盖，长期维护和 review 成本快速上升）

防守规则：

1. 后续新增 CALLS strategy 前，必须先评估是否拆到 helper / table / strategy module。
2. 如果暂不拆分，closure review 必须记录当前行数、为什么暂不拆、以及新增策略的测试证据。
3. 拆分只能做行为等价移动；禁止借拆分混入语义变化。
4. 大文件维护风险不阻塞当前 stats bug 修复，但阻止继续无界追加新方向。

---

### 0.1 External crate call stats hardcoded-zero bug

状态：**已修复（Rust-core `dda27b3`）**

修复（2026-05-04）：

- 根因：`output.rs` 中 stats 字段硬编码为 0。
- 修复：从 `call_list` 实时计算。
- 新增 test `external_crate_stats_are_computed`。
- 85/85 tests pass，c10: `callExternalCrateTotal=3`、`callExternalCrateClassified=3`。

风险级别：~~MEDIUM~~ → **已消除**

---

### 0. Graph schema v0.2 dangling CALLS edge

状态：**已修复（Rust-core `f1502a6`）**

修复（2026-05-04）：
- 根因：`inspect_project_model_with_options` 中，当 `include_graph && include_calls` 时，`include_symbols` 未被强制设置为 true，导致 `output.symbols` 为空，graph emitter 产出 CALLS edges 但无对应 symbol nodes。
- 修复：在 `output.rs` 的 `inspect_project_model_with_options` 中，当 `include_graph && include_calls` 时强制 `include_symbols = true`。
- 新增 test `test_graph_calls_without_symbols_flag_preserves_endpoint_integrity`：验证 `--include calls --include graph`（无 `--include symbols`）时每条 CALLS edge 的 source/target node 存在，且 symbol count >= 2。

验证：
- `cargo fmt --check` clean
- `cargo test` 84/84 pass（含新增 endpoint integrity test）
- `--include calls --include graph` on c1-same-module：CALLS=1, symbolNodes=2, danglingCalls=0

防守规则：

- ~~不继续扩展 schema / adapter / method / external crate 新方向，直到该 bug 修复。~~ bug 已修复，gate 解除。
- 不用“CALLS edge 暂不验证”作为 closure 理由。
- 不用“新增 expected-graph golden”作为 closure 理由，golden 可能固化错误输出。
- 若选择不输出 edge，则必须明确说明 no-edge policy；但更推荐补齐 symbol node contract。

---

## 一、CALLS 相关风险

### 1. CALLS 仍是 intermediate output

状态：**已知限制（已大幅缓解）**

- Graph emitter v0.3 已产 CALLS edge（schema v0.2 集成，v0.3 扩展 DESIGNATION/ACCESSES）
- Resolution rate: 54.0%（1189/2203 on gitnexus-rust-core，2026-05-04 v4 consolidation）
- 核心驱动：stdlib trait method (685, 57.6%) + external crate path (79) + receiver type (77) + same-module (263)
- Method call 占 69.5%（742 unresolved，stop-line：no full type inference）
- External crate 已支持 Phase 1 direct path resolution（std/core/alloc，confidence 0.80→0.85）+ sysroot index

防守规则：method dispatch 仅允许 low-confidence blind name heuristic；external crate 仅允许 dependency-name classification；不做 receiver type inference / trait solving / external crate API symbol resolution。

### 2. calls.rs 维护负担

状态：**已知限制**

- calls.rs 已增长到约 2053 行，超过预估 ~500 行
- 5 个独立 resolution strategy 各自完整实现
- 后续可抽取共享 helper 降低冗余

### 3. Same-file unique-name heuristic 语义限制

状态：**已验证，残留风险 LOW**

- 实际作用是 modulePath flat limitation 的弥补
- 在编译正确的 Rust 代码中不会独立触发跨模块调用
- 3/5 core fixture 不可编译（SF1/SF2/SF5，已标注 static-analysis-only）
- SF6 compile-valid fixture 覆盖真实场景

防守规则：confidence 0.70 不超过 same-module (0.90) 和 import (0.85)

---

## 二、数据模型风险

### 4. modulePath flat limitation

状态：**已知限制**

- Call site / ImportUse 的 modulePath 使用文件级路径（如 `crate::item`），不区分 inline module 内部
- 导致 same-module lookup 在 inline module 内失败
- Same-file heuristic 作为弥补，但 confidence 降低到 0.70
- Symbol extraction 已通过 ModulePathMap 精准化，但 call site 仍使用文件级

残留风险：inline module 内精确 modulePath 追踪推迟

### 5. CalleeIndex 与 SymbolIndex 数据冗余

状态：**已知限制**

- calls.rs 的 CalleeIndex 和 imports.rs 的 SymbolIndex 各自建 HashMap
- 两者都按 modulePath + name 索引 symbol
- 未抽取共享索引层

---

## 三、编译 / 平台风险

### 6. tree-sitter C binding 编译

状态：**已缓解**

- 需要 `cc` crate + C compiler
- CI 环境必须安装 C compiler
- Feature flag `tree-sitter-extraction` 控制，`--no-default-features` 可禁用
- TextItemExtractor 保留 fallback

残留风险：
- 某些平台 C compiler 缺失导致编译失败
- tree-sitter-rust grammar 更新滞后于 Rust edition

### 7. grammar 覆盖度

状态：**已知限制**

- tree-sitter-rust 0.24.2 可能不支持最新 Rust nightly 语法（如 `gen` blocks）
- 不支持语法产生 ERROR 节点，`tree-sitter-parse-error` diagnostic 标记
- macro 生成 item 不可见（proc macro / macro_rules!）
- cfg-gated item 状态不确定

---

## 四、Fixture 风险

### 8. Static-analysis-only fixture 语义

状态：**已缓解**

- SF1/SF2/SF5 是 static-analysis-only fixture，不反映 Rust 编译器语义
- 已标注 `// static-analysis-only: not compilable in Rust`
- SF3/SF4/SF6 是 compile-valid fixture
- C1-C7 全部 compile-valid

防守规则：不把不可编译 fixture 当成 Rust semantic truth

---

## 五、已知 Stop-line

以下内容是 Rust-core MVP 的明确 stop-line：

- No dangling graph edge（schema v0.2 CALLS edge source/target 必须存在）
- No full method dispatch（blind name heuristic only）
- No type inference / trait solving
- No external crate API symbol resolution（Phase 1: std/core/alloc direct path resolution, confidence 0.80; Phase 2: sysroot symbol index TBD）
- No macro expansion
- No full cfg evaluator
- No `cargo metadata` execution
- No proc-macro / build.rs execution
- No production replacement
- No UI / MCP server / commercial distribution
- No live repo modification
- No GitNexus-RC runtime/schema modification

---

## 每次开工前风险自问

1. 这次是否会触发 method dispatch / type inference / trait solving？
2. 是否依赖 external crate resolution？
3. confidence/reason 是否被诚实保留？
4. fixture 是 compile-valid 还是 static-analysis-only？
5. 是否依赖 modulePath flat limitation 的弥补行为？
