<!-- version: 1.0.0 -->
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
- **No graph CALLS edge** — Graph emitter v0 不产 CALLS edge，需 schema v0.2
- **No method dispatch** — `obj.method()` 只产 diagnostic（需 type inference）
- **No type inference / trait solving** — 不推断变量类型，不做 trait bound satisfaction
- **No external crate resolution** — `serde_json::to_string()` 等外部 crate 调用不解析
- **No macro expansion** — `foo!()` 不展开
- **No full cfg evaluator** — cfg-gated `mod` 只标记 `unknown`
- **No `cargo metadata` execution** — 只用 manifest-derived project model
- **No proc-macro / build.rs** — 不执行
- **No UI / MCP server / commercial distribution**
- **No live repo modification** — 不改 open-nwe / cangjie / warp / openfang 源码
- **No GitNexus-RC runtime/schema modification** — 不改 GitNexus-RC adapter / graph schema / package

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
| 2026-05-04 | 1.0.0 | Initial AGENTS.md for Rust-core minimum governance. |
