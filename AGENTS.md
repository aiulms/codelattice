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
- **Method dispatch remains low-confidence heuristic only** — 允许 blind method-name / explicit receiver-type annotation heuristic；禁止 full receiver type inference / trait solving
- **External crate support remains bounded** — 允许 std/core/alloc direct path 和 imported stdlib/prelude type 的有限解析；禁止任意 external crate API symbol resolution / sysroot index
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

状态：**已修复（Rust-core `f1502a6`，复核 OK）**

复核（2026-05-04，after Rust-core `8739f7a`）：

- `--include calls --include graph` on `c1-same-module`: `symbolNodes=2`、`CALLS=1`、`danglingEdges=0`
- 新增 endpoint integrity test 已通过。
- 若该 smoke 再次失败，必须重新打开本 gate。

### External crate call stats hardcoded-zero bug

状态：**ACTIVE，下一轮必须优先修复；不要继续扩大 receiver/method 新方向直到该 stats contract 修复。**

复现：

```bash
cargo run -q -p gitnexus-rust-core-cli -- project-model inspect \
  --root fixtures/call-resolution/c10-external-crate \
  --include calls
```

当前问题：

- `calls` 中已有 external crate calls，例如：
  - `std::vec::Vec::new` → `callKind="external-crate"`、`knownCrate="std"`、`reason="call-external-crate-path-resolved"`
  - `std::collections::HashMap::<&str, i32>::new` → `knownCrate="std"`
  - `std::path::PathBuf::new` → `knownCrate="std"`
- 但 `stats.callExternalCrateTotal == 0` 且 `stats.callExternalCrateClassified == 0`
- 根因候选：`output.rs` 中 `Stats { call_external_crate_total: 0, call_external_crate_classified: 0 }` 仍硬编码。

修复要求：

1. 不要删除 stats 字段或用 closure 文档解释掉该问题。
2. `stats.callExternalCrateTotal` 必须从 `call_list` 计算，至少覆盖 `callKind == "external-crate"`。
3. `stats.callExternalCrateClassified` 必须从 `call_list` 计算，至少覆盖 `knownCrate.is_some()` 的 external crate calls。
4. 新增或更新 call comparison / CLI test：`c10-external-crate` 的 stats 必须为非零，并和 actual calls 一致。
5. `cargo fmt --check` + `cargo test` 必须全绿。

### Large source file quality watch

状态：**ACTIVE quality watch；不是 stop-line，但继续扩大 CALLS 方向前必须显式处理。**

当前观察（2026-05-04）：

- `crates/project-model/src/calls.rs` 已增长到约 2053 行，且仍在继续承载新 resolution strategy。
- `docs/RISK_LEDGER.md` 中旧记录仍写 `calls.rs ~1400 行`，已过期。
- 这不是单纯行数问题，而是 extractor / resolver / stdlib tables / text fallback / diagnostics / fixture policy 混在同一文件中，后续维护者难以判断改动边界。

质量要求：

1. 继续新增 CALLS strategy 前，必须先判断能否抽出小型 helper / table / strategy section；不能无界追加到 `calls.rs`。
2. 如果暂不拆分，closure review 必须说明为什么暂不拆，并记录当前行数与新增复杂度。
3. 新增逻辑必须带 fixture/harness 或 CLI smoke；不能只靠真实项目统计证明。
4. 不允许通过大范围重排来掩盖语义变更；拆分应保持行为等价并先跑全量测试。

## Verification

```bash
cargo fmt --check    # Formatting check
cargo test           # All tests
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
