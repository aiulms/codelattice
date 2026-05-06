# Rust-core Plans Index

最后更新：2026-05-06（Phase 2 Slice 1 完成：cangjie crate skeleton + cjpm parser）

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

**Phase 2 Slice 2 — workspace/dependency metadata（待用户 gate）**

当前进度：Slice 1 已完成（cangjie crate skeleton + cjpm parser, 105 tests pass）。
下一步：扩展 workspace member 递归解析、build-members/test-members 过滤、cjpm.lock 最小 parser、path dependency resolution helper。

详细 Execution Card 待写（Slice 2）。

### 路线收束（2026-05-06）

GitNexus 路线已收束：GitNexus-RC TS 冻结为过渡生产环境；gitnexus-rust-core 确认为真正 Rust-native 复刻主线。

详细审计：[GitNexus-RC 路线收束审计](https://gitcode.com/aiulms/gitnexus-rc) — `docs/language-support/plans/2026-05-06-gitnexus-rust-native-mainline-convergence-audit.md`

### Rust-native Cangjie Migration

**Phase 1 — Preflight ✅ 完成（2026-05-06）：**
- 冻结 Cangjie adapter scope：~3,500 行可移植逻辑 vs GitNexus-specific 管道
- 设计 Rust-core cangjie crate 结构（manifest/extractors/resolver/diagnostics/project）
- 确定 subprocess 策略（cjc/cjlint/cjpm tree → spawn）+ tree-sitter 策略（vendor Option A + feature gate）
- 定义 stop-line + Phase 2 implementation scope 预览
- Preflight：[GitNexus-RC plans](https://gitcode.com/aiulms/gitnexus-rc) — `docs/language-support/plans/2026-05-06-rust-native-cangjie-migration-phase1-preflight.md`

**Phase 2 — Implementation（进行中，2026-05-06）：**

Slice 1 — cangjie crate skeleton + cjpm parser ✅ 完成：
- 新建 `crates/cangjie` crate，加入 workspace
- 实现 `parse_cjpm_toml()` / `load_cjpm_manifest()` API
- 使用已有 `toml` crate（零新增依赖），serde 反序列化
- 支持 [package]（name/version/src-dir/cjc-version/output-type）、[workspace]（members/build-members）、[dependencies]（simple string + inline table + git）
- 新增 fixture `fixtures/cangjie/cjpm-basic/` + 15 tests（13 unit + 2 integration）
- Execution Card：gitnexus-rust-core `docs/plans/2026-05-06-cangjie-phase2-slice1-execution-card.md`

**Phase 2 Slice 2 — workspace/dependency metadata（待 gate）：**
1. 解析 workspace member 的 cjpm.toml 递归
2. build-members / test-members 过滤
3. cjpm.lock 最小 parser（placeholder preflight）
4. path dependency resolution helper

**Phase 2 Slice 3 — baseline project model output（后续）：**
- project root detection via cjpm.toml
- list source files under src-dir
- emit minimal project metadata

**Phase 2 Slice 4 — tree-sitter Cangjie preflight / vendor gate（后续）：**
- 不直接 vendor 大 parser.c，先写 vendor gate / feasibility doc
- 检查 tree-sitter-cangjie 来源、license、ABI、编译方式
- 设计 feature gate

**后续 slices（5+）：**
5. cjc/cjlint diagnostics runner
6. LSP client（future，P1）
7. Graph emitter 扩展（Diagnostic + ANNOTATES + MODIFIES）

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
1. ~~Cangjie migration preflight（B 线下一轮 opening）~~ ✅ 完成（Phase 1 preflight）
2. ~~Phase 2 Slice 1 — cangjie crate skeleton + cjpm parser~~ ✅ 完成
3. Phase 2 Slice 2 — workspace/dependency metadata（需用户 gate）
4. Rust-core Rust analysis readiness 改善（CALLS resolution rate 等 bounded slices）
5. 按 tracker 优先级选择下一轮 opening
