# GitNexus Rust-core 风险记录

最后更新：2026-05-04

## 用途

本文件记录 Rust-core 的已知风险、残留限制和防守规则。
完整风险记录（含 GitNexus-RC 侧风险）见 GitNexus-RC `docs/language-support/RISK_LEDGER.md`。

---

## 零、Active Bug Gate

### 0. Graph schema v0.2 dangling CALLS edge

状态：**ACTIVE，下一轮必须优先修复**

复现：

```bash
cargo run -q -p gitnexus-rust-core-cli -- project-model inspect \
  --root fixtures/call-resolution/c1-same-module \
  --include calls \
  --include graph
```

当前观察：

- `schemaVersion=0.2.0`
- `edges` 中存在 `CALLS`
- `CALLS` edge 的 `source` / `target` 指向 `symbol:*`
- `nodes` 中没有对应 `symbol:*` node

风险级别：**HIGH**（graph 输出违反 edge endpoint integrity）

根因候选：

- `--include calls` 会为解析 call 内部提取 symbols
- 但未显式 `--include symbols` 时，`ProjectModelOutput.symbols` 被置空
- graph emitter 根据 `output.calls` 产 CALLS edge，却没有 symbol nodes 可引用

修复门槛：

1. `--include calls --include graph` 对 `fixtures/call-resolution/c1-same-module` 必须同时满足：
   - 有至少 1 条 `CALLS` edge
   - 每条 `CALLS` edge 的 source/target node 都存在
2. 新增或更新 graph test 覆盖该组合。
3. `cargo fmt --check` + `cargo test` 全绿。

防守规则：

- 不继续扩展 schema / adapter / method / external crate 新方向，直到该 bug 修复。
- 不用“CALLS edge 暂不验证”作为 closure 理由。
- 若选择不输出 edge，则必须明确说明 no-edge policy；但更推荐补齐 symbol node contract。

---

## 一、CALLS 相关风险

### 1. CALLS 仍是 intermediate output

状态：**已知限制**

- Graph emitter v0.2 已产 CALLS edge，但 endpoint integrity 必须优先修复（见 Active Bug Gate）
- 整体 resolution rate 仅 6.4%（4 项目 133,885 calls 统计）
- Method call 占 62%，是绝对主导的 call form（当前仅 blind name heuristic，不做 receiver type inference）
- External crate 调用占 27%，当前仅 dependency-name classification，不解析 external crate API symbol

防守规则：method dispatch 仅允许 low-confidence blind name heuristic；external crate 仅允许 dependency-name classification；不做 receiver type inference / trait solving / external crate API symbol resolution。

### 2. calls.rs 维护负担

状态：**已知限制**

- calls.rs ~1400 行，超过预估 ~500 行
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
- No external crate API symbol resolution（classification only）
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
