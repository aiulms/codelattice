# Rust-core Plans Index

最后更新：2026-05-06（GitNexus 路线收束：Rust-core 确认为 Rust-native 复刻主线）

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

**（当前没有进行中的 execution card）**

### 路线收束（2026-05-06）

GitNexus 路线已收束：GitNexus-RC TS 冻结为过渡生产环境；gitnexus-rust-core 确认为真正 Rust-native 复刻主线。

详细审计：[GitNexus-RC 路线收束审计](https://gitcode.com/aiulms/gitnexus-rc) — `docs/language-support/plans/2026-05-06-gitnexus-rust-native-mainline-convergence-audit.md`

### Rust-native Cangjie Migration（future，待开启）

此为 B 线核心内容。在当前 Rust-core 能力基础上，逐步将 Cangjie adapter 从 TS 迁移到 Rust-native 实现。

**Phase 1 — Preflight（docs-only，下一轮 opening）：**
1. 写 Cangjie migration preflight：冻结 Rust-core Cangjie adapter scope
2. 设计 Cangjie crate 结构（crates/cangjie/）
3. 确定 cjc/cjlint/cjpm subprocess 策略
4. 确定 manifest model（cjpm.toml）
5. 冻结符号提取 + reference index scope
6. 冻结 diagnostics pipeline scope
7. 确定 cjpm import resolver scope（复用 3-tier strategy）

**Phase 2 — Implementation（后续 execution cards）：**
1. cjpm manifest parser（toml crate）
2. Cangjie tree-sitter adapter
3. cjc/cjlint diagnostics runner
4. LSP client（future，P1）
5. Graph emitter 扩展（Diagnostic + ANNOTATES + MODIFIES）

**Rust-core stop-line 重申（不可协商）：**
- No MCP server
- No HTTP API
- No UI
- No embeddings
- No type inference / trait solving
- No macro expansion

### 当前活跃维护

CALLS large-file maintenance preflight 已完成并进入 implementation：
- stdlib_tables.rs 已提取（calls.rs 2161→1858，-14.0%），89/89 tests pass，零 golden drift。
- Text fallback / CalleeIndex 提取留待第二刀。

下一步优先级：
1. GitNexus-RC tracker / plans README / RISK_LEDGER 同步 ✅（2026-05-06 路线收束已完成）
2. Cangjie migration preflight（B 线下一轮 opening）
3. 选择下一轮 opening（按 tracker 优先级：active bug gate → next opening → quality watch）
