# Rust-core Plans Index

最后更新：2026-05-06（Phase 2 Slice 7 完成：Cangjie graph output，142 tests pass）

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

**Phase 2 Slice 7 — Cangjie graph output ✅ 完成（2026-05-06）**

当前进度：Slice 1-7 已完成。
Slice 7：方案 B2（cangjie 独立 graph output）已实现，142 tests pass，零新增依赖。
新增 `crates/cangjie/src/graph.rs`：CangjieGraphOutput（Repository/Package/SourceFile/Symbol 节点 + ContainsPackage/OwnsSource/Defines 边）。
Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice7-preflight.md`
Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice7-execution-card.md`

**Phase 2 Slice 8+ — 下一步（需 preflight)：**
- diagnostics runner（cjc/cjlint subprocess）或 LSP client — 均触发 stop-line，需先写 preflight

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

Slice 2 — workspace/dependency metadata ✅ 完成：
- `resolve_workspace_manifest()`：递归加载 workspace member 的 cjpm.toml
- `active_members()`：build-members 过滤（对齐 TS 行为）
- `resolve_path_dependency()`：path-based dep 解析为绝对路径
- `parse_cjpm_lock()` / `load_cjpm_lock()`：cjpm.lock 最小 parser（[[requires]] entries）
- 新增 fixture `fixtures/cangjie/cjpm-workspace/` + 11 tests（8 unit + 3 integration）
- 116/116 tests pass
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice2-execution-card.md`

Slice 3 — baseline project model output ✅ 完成：
- `find_project_root()`：从路径向上查找 cjpm.toml
- `list_source_files()`：递归列出 .cj 文件（跳过 hidden/target/.cache/.generated）
- `build_project_model()`：从 workspace 构建 project model
- `CangjieProject` / `CangjiePackageInfo` 类型定义
- 7 new unit tests，123/123 tests pass
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice3-execution-card.md`

Slice 4 — tree-sitter Cangjie vendor gate ✅ 完成（docs-only）：
- 上游来源审计：gitcode.com/Cangjie-SIG/tree-sitter-cangjie（Mulan PSL v2.0）
- License 评估：与 MIT 兼容
- ABI 分析：ABI 14，tree-sitter 0.26 预期兼容
- 编译方案：`cc` crate + build.rs + feature gate `tree-sitter-cangjie`
- 风险评估：~4.7MB parser.c，已有先例（tree-sitter-rust ~3.5MB）
- 替代方案：text-level regex fallback（能力上限低）/ 等待上游 crates.io（不确定）
- 推荐：选项 A（批准 vendor + feature gate，进入 Slice 5）
- Vendor Gate：`docs/plans/2026-05-06-cangjie-phase2-slice4-vendor-gate.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice4-execution-card.md`

**Phase 2 Slice 5 — tree-sitter Cangjie 集成 ✅ 完成：**
- Vendor parser.c (~4.7MB) + scanner.c (~5.7KB) + tree_sitter headers 从 GitNexus-RC 复制
- 新增 `build.rs`：`cc::Build` 编译 parser.c + scanner.c（仅 feature 启用时）
- Feature gate `tree-sitter-cangjie`：`tree-sitter = "0.26"` + `cc = "1"`，默认关闭
- 新增 `crates/cangjie/src/extractors/mod.rs`：`try_init_cangjie_parser()` / `is_cangjie_parser_available()` / `parse_cangjie_source()` / `tree_has_error_nodes()`
- `pub mod extractors` 条件导出（feature-gated）
- 新增 smoke test：`tests/tree_sitter_smoke.rs`（3 tests，feature-gated）
- `cargo build` 成功（零新增编译，feature 关闭时）
- `cargo build --features tree-sitter-cangjie` 成功（parser.c 编译通过，4 个上游 scanner.c warnings）
- `cargo test` 126/126 pass（123 existing + 3 new）
- `cargo test --features tree-sitter-cangjie` 126/126 pass（smoke test 通过，parse main.cj 无 ERROR nodes）
- 零新增依赖（tree-sitter 0.26 已在 workspace；cc crate 已在 lockfile）
- 不改 GitNexus-RC runtime / Tool / live repo
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice5-execution-card.md`

Slice 6 — tree-sitter Cangjie AST symbol extraction ✅ 完成：
- 7 种符号类型：Function / Class / Struct / Enum / Interface / TypeAlias / Macro
- 基于 tree_sitter::Query + StreamingIterator API
- 新增 `extract_cangjie_symbols()` / `extract_cangjie_symbols_from_tree()` API
- Query 对齐 tags.scm 符号模式（含 `mainDefinition` / `main` anonymous node 特殊处理）
- 已知限制：`macro` 函数定义语法不被当前 grammar 支持（仅 `macro package` 声明）
- 新增 9 tests，135/135 pass，零新增依赖
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice6-execution-card.md`

Slice 7 — Cangjie graph output ✅ 完成（2026-05-06）：
- 方案 B2（cangjie 独立 graph output）已实现
- 新增 `crates/cangjie/src/graph.rs`（~370 行）
- CangjieGraphOutput：Repository/Package/SourceFile/Symbol 节点 + ContainsPackage/OwnsSource/Defines 边
- 新增 `inspect_cangjie_project()` 一站式入口
- 6 tests，142/142 pass（with feature），零新增依赖
- 不改 project-model crate
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice7-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice7-execution-card.md`

**后续 slices（8+）：**
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
3. ~~Phase 2 Slice 2 — workspace/dependency metadata~~ ✅ 完成
4. ~~Phase 2 Slice 3 — baseline project model output~~ ✅ 完成
5. ~~Phase 2 Slice 4 — tree-sitter Cangjie vendor gate~~ ✅ 完成（docs-only，待用户批准）
6. ~~Phase 2 Slice 5 — tree-sitter Cangjie 集成~~ ✅ 完成
7. ~~Phase 2 Slice 6 — tree-sitter Cangjie AST symbol extraction~~ ✅ 完成
8. Phase 2 Slice 7 — Cangjie graph output ✅ 完成
9. Phase 2 Slice 8+ — diagnostics runner / LSP client（需 preflight）
10. Rust-core Rust analysis readiness 改善（CALLS resolution rate 等 bounded slices）
