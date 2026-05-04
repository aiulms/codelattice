# Rust-core Plans Index

最后更新：2026-05-04

## 用途

本目录存放 Rust-core 的计划文档（preflight / execution card / closure review）。

日常 implementation closure 可在本地记录，也可记录到 GitNexus-RC `docs/language-support/plans/`。
跨仓 handoff / milestone 必须记录到 GitNexus-RC。

## 命名

- `YYYY-MM-DD-<topic>-preflight.md`
- `YYYY-MM-DD-<topic>-execution-card.md`
- `YYYY-MM-DD-<topic>-closure-review.md`

## 治理来源

- [GitNexus-RC TASK_TRACKER](https://gitcode.com/aiulms/gitnexus-rc) (`docs/language-support/TASK_TRACKER.md`)
- [GitNexus-RC RISK_LEDGER](https://gitcode.com/aiulms/gitnexus-rc) (`docs/language-support/RISK_LEDGER.md`)
- [GitNexus-RC GOVERNANCE](https://gitcode.com/aiulms/gitnexus-rc) (`docs/language-support/GOVERNANCE.md`)

## 当前推荐下一篇计划

**CALLS large-file maintenance preflight**

- 背景：`crates/project-model/src/calls.rs` 已超过 2000 行，继续承载 extractor / resolver / stdlib tables / text fallback / diagnostics / fixture policy / 多条 resolution strategy。
- 目标：先评估 behavior-preserving 拆分 / helper / table module / strategy module；如果暂不拆，必须记录理由和下一次触发条件。
- 边界：不混入 CALLS 语义变更；不改 graph schema / adapter / Cargo.toml；拆分必须保持行为等价并跑全量验证。
- 验证建议：`cargo fmt --check`、`cargo test`、call fixture comparison、graph fixture comparison、`--include calls --include graph` endpoint integrity smoke。
