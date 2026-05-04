<!-- version: 1.1.0 -->
<!-- Last updated: 2026-05-04 -->

Last reviewed: 2026-05-04

**Project:** GitNexus Rust-core · **Environment:** dev · **Governance source:** [GitNexus-RC](https://gitcode.com/aiulms/gitnexus-rc)

## Purpose

GitNexus Rust-core 是 GitNexus 的 Rust 语言分析核心实现。它不是 GitNexus-RC 的替代发布版，而是独立的 Rust 工具链。

**治理关系：**
- GitNexus-RC 是治理来源、架构决策记录和 TypeScript adapter 主仓库
- Rust-core 是 Rust 实现主体
- 所有语言支持决策、fixture 设计、confidence/reason 策略源自 GitNexus-RC `docs/language-support/`
- Rust-core 日常 implementation closure 可在本地记录；跨仓 handoff / milestone 记录到 GitNexus-RC

## Scope

| Boundary | Rule |
|----------|------|
| **Reads** | `crates/`, `fixtures/`, `docs/`, `Cargo.toml` |
| **Writes** | `crates/`, `fixtures/`, `docs/` |
| **Executes** | `cargo fmt`, `cargo test`, `cargo run`, `rustc` (fixture validation) |
| **Off-limits** | GitNexus-RC runtime/adapter/schema, live repos (open-nwe/cangjie), production analyze |

## Execution Sequence (complex tasks)

For multi-step work：
1. 先做 preflight（设计 + 风险评估），记录到 `docs/plans/`
2. 冻结 execution card（write set / forbidden set / stop-line）
3. Implementation
4. Closure review
5. 如涉及跨仓变更，同步记录到 GitNexus-RC

**跨仓操作规则：**
- 修改 Rust-core 后必须 `cargo fmt --check` + `cargo test` + `git diff --check`
- Commit 后 push gitcode master
- Push 失败时记录错误，继续后续低风险工作
- 不做 destructive git 操作

## Stop-lines (MVP)

以下内容是 Rust-core MVP 的明确 stop-line：

- **No production replacement** — Rust-core 不是 GitNexus-RC TypeScript adapter 的替代
- **Graph CALLS edge must not be dangling** — schema v0.2 可产 CALLS edge，但 source/target 必须指向已存在 node
- **Method dispatch remains low-confidence heuristic only** — 允许 blind method-name resolution；禁止 receiver type inference / trait solving
- **External crate is classification only** — 允许 dependency-name classification；禁止 external crate API symbol resolution
- **No type inference / trait solving** — 不推断变量类型，不做 trait bound satisfaction
- **No macro expansion** — `foo!()` 不展开
- **No full cfg evaluator** — cfg-gated `mod` 只标记 `unknown`
- **No `cargo metadata` execution** — 只用 manifest-derived project model
- **No proc-macro / build.rs** — 不执行
- **No UI / MCP server / commercial distribution**
- **No live repo modification** — 不改 open-nwe / cangjie / warp / openfang 源码
- **No GitNexus-RC runtime/schema modification** — 不改 GitNexus-RC adapter / graph schema / package

## Active Bug Gate

### Graph schema v0.2 CALLS dangling-edge bug

状态：**ACTIVE，下一轮必须优先修复，不要继续扩展新方向。**

最新复核（2026-05-04，after Rust-core `7cb67de`）：

- `c1-same-module expected-graph.json` 已新增，但只是把 dangling CALLS edge 纳入 golden；**bug 仍存在**。
- smoke 结果仍是 `CALLS=1`、`symbolNodes=0`、`danglingCalls=1`。
- 不允许把“新增 expected-graph golden”或“CALLS edge 数量验证”作为修复完成标准。
- 必须新增 endpoint integrity assertion：每条 graph edge 的 `source` / `target` 都必须存在于 `nodes[].id`。

复现：

```bash
cargo run -q -p gitnexus-rust-core-cli -- project-model inspect \
  --root fixtures/call-resolution/c1-same-module \
  --include calls \
  --include graph
```

当前问题：

- 输出 `schemaVersion=0.2.0`
- 输出含 `CALLS` edge，例如 `symbol:c1-same-module::crate::main_fn -> symbol:c1-same-module::crate::helper`
- 但 `nodes` 中没有对应 `symbol:*` node，形成 dangling edge

根因候选：

- `--include calls` 内部会提取 symbols 用于解析 calls
- 但 `include_symbols=false` 时 `ProjectModelOutput.symbols` 被置空
- graph emitter 仍根据 `output.calls` 产 `CALLS` edge，于是 edge 引用不存在的 symbol node

修复要求：

1. 修复 dangling edge；不要用文档化代替修复。
2. 推荐策略：当 `include_graph && include_calls` 时，graph 输入必须保留/获得 symbol nodes；或者 graph emitter 必须在 source/target symbol node 存在时才产 CALLS edge。优先选择 contract 更一致、可测试的方案。
3. 新增或更新 graph test：`--include calls --include graph` 对 `c1-same-module` 必须产 `CALLS` edge，且 source/target node 都存在。
4. 新增通用 graph endpoint integrity test：所有 graph fixtures / smoke 中每条 edge 的 source/target node 都存在。
5. `cargo fmt --check` + `cargo test` 必须全绿。
6. 修复后写 closure review，并同步更新 Rust-core / GitNexus-RC tracker。

## Verification

```bash
cargo fmt --check    # Formatting check
cargo test           # All tests (currently 81 tests)
```

## Comment Policy

语义代码注释默认使用中文，必要英文术语保留。必须注释的区域：
- ambiguous / no-edge / low-confidence / fallback 逻辑
- dedupe / project ownership / graph policy
- stop-line / absence assertion
- confidence/reason 值的选择理由

## Reference Docs

- `docs/architecture/` — 架构文档（graph schema / output contract / confidence policy / project model）
- `docs/decisions/` — 决策记录（command authority / known limitations / no-edge policy）
- [GitNexus-RC `docs/language-support/`](https://gitcode.com/aiulms/gitnexus-rc) — 治理来源（TASK_TRACKER / RISK_LEDGER / GOVERNANCE / plans）

## Changelog

| Date | Version | Change |
|------|---------|--------|
| 2026-05-04 | 1.1.0 | Added active bug gate for graph schema v0.2 dangling CALLS edges; refreshed CALLS/method/external stop-lines to match landed reality. |
| 2026-05-04 | 1.0.0 | Initial AGENTS.md for Rust-core minimum governance. |
