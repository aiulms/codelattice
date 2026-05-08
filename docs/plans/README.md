# Rust-core Plans Index

最后更新：2026-05-08（Docs consolidation: QUALITY.md Rust gates + README stale stats fix）

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

## 当前状态总结

**Cangjie 线：** Production Acceptance Stages 1-3 ✅ 完成。0 synthetic, 0 duplicate, 0 dangling, deterministic。graph_contract 24/24, multi_project_smoke 4/4 fixture + 4 production, cangjie_inspect 18/18。已稳定为本地生产试用候选。

**Rust 线：** Resolution rate 65.7%（2338/3557）。0 dangling CALLS edges。Graph contract 23/23（3 fixtures）。Enum constructor resolution + external symbol node completion + cross-file same-crate resolution + wildcard disambiguation 已落地。method-calls 仍为主要 gap（~1217 unresolved，stop-line: no type inference）。

## 当前推荐下一篇计划

**Priority 2 续 — Rust CALLS resolution quality**
- `crate::` 多段路径分类修复（associated-function 误分类为 qualified-path，1-2 calls）
- 关联函数 resolution：16 unresolved（含 derive-generated 方法、外部 crate type 方法、re-export 路径）
- 低置信度 reason/confidence 矩阵审计
- call form 文档与 confidence 矩阵对齐

**Priority 3 续 — Rust graph contract**
- 第 4 个 contract fixture（如 module-hierarchy 或 workspace-member）
- 匹配 Cangjie 的 4 fixture 覆盖水平

**Priority 4 — Cangjie maintenance**
- Quality gate 周期性回归验证
- QUALITY.md 维护
- 小范围 regression fix
- 小范围 regression fix

**Phase 2 Slice 8 — Cangjie diagnostics runner ✅ 完成（2026-05-06）：**
- 实现 cjc/cjlint subprocess diagnostics runner（方案 A）
- 新增 `crates/cangjie/src/diagnostics/types.rs`：CangjieDiagnostic + DiagnosticSeverity（对齐 TS NormalizedDiagnostic）
- 新增 `crates/cangjie/src/diagnostics/runner.rs`（~450 行）：SDK tool discovery + cjc runner + cjlint runner + JSON 解析
- SDK tool 解析：CANGJIE_HOME → CANGJIE_SDK_HOME → PATH 优先链
- 1-based → 0-based 坐标归一化，graceful degrade（SDK absent 时返回空 Vec）
- 60s subprocess timeout with try_wait polling loop
- graph.rs 新增 Diagnostic NodeKind + Annotates EdgeKind + emit_cangjie_diagnostics()
- 21 new tests：2 types + 16 runner + graph diagnostics coverage，163/163 pass（with feature）
- 零新增依赖（std::process::Command stdlib + serde_json 已有）
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice8-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice8-execution-card.md`

**Phase 2 Slice 9 — diagnostics integration into inspect_cangjie_project ✅ 完成（2026-05-06）：**
- 将 `run_all_diagnostics()` + `emit_cangjie_diagnostics()` 串联到 `inspect_cangjie_project()` one-shot 中
- SDK absent 时 graceful degrade（空 Vec），不影响现有符号/图输出
- 图输出自动包含 Diagnostic nodes + ANNOTATES edges
- 零新增依赖，163/163 pass（with feature），不改 Tool / live repo
- ~22 行 graph.rs 变更

**Phase 2 Slice 10 — same-file reference extraction ✅ 完成（2026-05-06）：**
- Same-file AST walk reference extraction（USES/ACCESSES/MODIFIES edges）
- 新增 `crates/cangjie/src/extractors/references.rs`：AST walk + same-file symbol index
- Port TS adapter `extractReferences()` 模式：typeStack + funcStack 跟踪
- Builtin type 过滤（25 种 Cangjie builtin types 不产生 USES edge）
- graph.rs 新增 Uses/Accesses/Modifies EdgeKind + emit_cangjie_reference_edges()
- 集成到 inspect_cangjie_project() one-shot
- 175/175 pass（with feature），155/155（without feature），零新增依赖
- 新增 fixture `fixtures/cangjie/references-basic/`
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice10-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice10-execution-card.md`

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

**Phase 2 Slice 11 — Cangjie import resolution ✅ 完成（2026-05-06）：**
- AST import 语句解析（feature-gated）+ same-project import resolution
- 新增 `crates/cangjie/src/extractors/imports.rs`（~610 行）：import 类型定义 + 字符串解析器（port TS adapter）+ AST walk + 候选目录生成 + 包名解析
- `parse_import_targets()` / `parse_named_import_candidates()` / `resolve_import_target()` API
- graph.rs 新增 Imports EdgeKind + `emit_cangjie_import_edges()` + `inspect_cangjie_project()` 集成
- 新增 fixture `fixtures/cangjie/imports-basic/` + 35 new tests（25 unit + 10 integration）
- 209/209 pass（with feature），179/179（without feature），零新增依赖
- 不支持 cjpm tree 子进程（deferred to Slice 11b），不解析 git-based dependency
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice11-preflight.md`

**Phase 2 Slice 11b — cjpm tree + external dependency resolution ✅ 完成（2026-05-06）：**
- 新增 `subprocess/cjpm_tree.rs`（~360 行）：port TS cjpm-metadata.ts
- `run_cjpm_tree()`：spawn `cjpm tree --skip-script`，30s timeout，graceful degrade
- `parse_cjpm_tree_output()`：two-phase parser（flat entries → index-path tree assembly）
- `find_package_dir_by_name()`：递归 workspace subtree 搜索（MAX_DEPTH=3）
- `resolve_tree_dependency_dir()`：thread-local cache + multi-root 聚合
- 新增 `ResolutionKind::TreeDependency` + `candidate_package_dirs()` 4-level fallback：
  workspace member → path dep → lock entry → tree dep
- `is_tree_dependency_match()` helper for external dep matching
- `resolve_cangjie_tool()` / `build_cangjie_spawn_env()` 改为 pub 复用
- 10 unit tests + graceful degrade tests，105/105 pass（with feature）
- 零新增依赖（std::process::Command stdlib + HashMap/PathBuf 标准库）
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice11b-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice11b-execution-card.md`

**Phase 2 Slice 12 — cross-file reference extraction ✅ 完成（2026-05-06）：**
- 从 same-file reference extraction 扩展到跨文件：通过 import binding table 解析被导入符号的 target file
- 新增 `CrossFileSymbolIndex`（project-wide symbol lookup）+ `ImportBindingTable`（import → target file 映射）
- `push_reference()` 两步 fallback：same-file（SameFileIndex, confidence 0.60-0.85）→ cross-file（ImportBindingTable, confidence 0.85）
- `CangjieReference` 新增 `target_file: Option<String>` 字段支持跨文件目标
- `emit_cangjie_reference_edges()` 使用 `target_file` 进行跨文件 symbol lookup
- `inspect_cangjie_project()` 重构：先提取 imports → 构建 ImportBindingTable → 再提取 references
- 新增 fixture `fixtures/cangjie/reference-cross-file-basic/` + 3 integration tests
- 108/108 pass（with feature），95/95（without feature），零新增依赖
- MVP 支持：explicit named/grouped import 的 type annotation reference
- 不支持：wildcard import expansion, alias renamed import, function call references, method dispatch
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-execution-card.md`
- Closure Review：`docs/plans/2026-05-06-cangjie-phase2-slice12-cross-file-reference-closure-review.md`

**Phase 2 Slice 13 — function call reference extraction ✅ 完成（2026-05-06）：**
- 关闭 function call gap：`postfixExpression` 含 `callSuffix` 的节点现在产生 USES edges
- 新增 `has_call_suffix()` + `extract_callee_name()` helpers：处理 simple call / constructor call / qualified call
- Method call 检测并跳过（`obj.method()` → 返回 None，需 receiver type inference）
- Builtin type 过滤（`Array(10)` 等构造函数调用不产生 USES edge）
- 嵌套调用支持（handler 不 return early，递归 walk 处理 `foo(bar())`）
- 复用现有 `push_reference()` pipeline：same-file（SameFileIndex, confidence 0.80）→ cross-file（ImportBindingTable, confidence 0.85）
- 新增 2 个 fixtures：`reference-function-call-basic/` + `reference-function-call-cross-file/`
- 10 integration tests，233/233 pass（with feature），0 fail，零新增依赖
- MVP 支持：simple function call / constructor call / qualified call / cross-file via explicit import
- 不支持：method call / wildcard import call / alias renamed import call / external dependency call
- Preflight：`docs/plans/2026-05-06-cangjie-phase2-slice13-function-call-reference-preflight.md`
- Execution Card：`docs/plans/2026-05-06-cangjie-phase2-slice13-function-call-reference-execution-card.md`
- Closure Review：`docs/plans/2026-05-06-cangjie-phase2-slice13-function-call-reference-closure-review.md`

**Phase 2 Slice 16 — Cangjie graph output parity smoke ✅ 完成（2026-05-07）：**
- 实现基础 parity smoke test (`crates/cangjie/tests/graph_parity_smoke.rs`)
- 验证节点/边类型覆盖率：Repository/Package/SourceFile/Symbol + 所有边类型
- 验证图结构完整性：所有边引用有效节点
- 验证输出确定性：多次运行结果一致
- 验证 JSON 序列化：可正确序列化/反序列化
- 6 new tests，248/248 pass（with feature），零新增依赖
- 不做 live repo/GitNexus-RC runtime 修改
- Preflight：`docs/plans/2026-05-07-cangjie-phase2-slice16-graph-output-parity-smoke-preflight.md`
- Execution Card：`docs/plans/2026-05-07-cangjie-phase2-slice16-graph-output-parity-smoke-execution-card.md`

**Phase 2 Slice 17 — Cangjie CLI surface MVP ✅ 完成（2026-05-07）：**
- 新增 CLI 子命令：`cangjie inspect` 和 `cangjie graph`
- Feature gate：`tree-sitter-cangjie` 启用时可见 Cangjie 子命令
- Graceful failure：cangjie 子命令始终可见，feature 禁用时返回清晰错误
- 稳定 JSON 输出：stdout 纯 JSON，stderr 承载错误
- 错误处理：root 不存在时退出码非零，错误信息清晰；feature 禁用时提示 `--features tree-sitter-cangjie`
- CLI integration tests：15 tests（13 feature-enabled + 2 feature-disabled）验证 JSON 契约、节点/边类型、错误路径
- Integration tests feature gating：`graph_parity_smoke.rs` + `alias_reference.rs` 添加 `#![cfg(feature = "tree-sitter-cangjie")]`
- 更新 README.md：添加 Cangjie CLI 使用说明、feature 要求、stop-lines
- 零新增依赖（复用 clap + serde_json + gitnexus-cangjie）
- 233/233 tests pass（with feature），45/45 pass（without feature）
- 不改 GitNexus-RC runtime/Tool/live repo
- Feature-gate follow-up ✅ 完成（2026-05-07）：CLI graceful failure, integration tests feature gating
- Preflight：直接从 Slice 17 spec 实现（execution card 即实现）
- Execution Card：本 slice 实现
- Follow-up：`fix(cangjie): make CLI feature gating graceful` (a87ea7e)

**Phase 2 Slice 18 — Cangjie production fixture smoke ✅ 大部分成功（2026-05-07）：**
- 在真实 Cangjie 项目（cangjie-GitNexus-Index/runtime/cjgui）上运行 smoke test
- 验证 CLI 可用性：✅ 成功运行，输出合法 JSON，运行时间 ~0.15s
- 统计基础指标：✅ Nodes=715（1 repo + 1 pkg + 14 files + 699 symbols），Edges=3,401（1 contains + 14 owns + 699 defines + 2,687 uses）
- 图结构完整性：❌ 发现 125 unique dangling source IDs，770 dangling source edges（23% 的 edges 损坏，均为构造函数调用）
- 输出确定性：✅ 两次运行结果一致
- Gap 分析：Reference extraction 为构造函数调用创建 edges，但 Symbol extraction 未提取构造函数 symbols，导致 ID 策略不一致
- 修复建议：Phase 2 Slice 19 — Reference source endpoint integrity repair（已完成）
- 实际方案：Synthetic callable source nodes（非完整 constructor symbol extraction）
- Future: 真实 constructor symbol extraction 需新 preflight
- 192/192 tests pass（without feature），259/259 pass（with feature）
- 不改 GitNexus-RC runtime/Tool/live repo
- Preflight：`docs/plans/2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-preflight.md`
- Execution Card：`docs/plans/2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-execution-card.md`
- Closure Review：`docs/plans/2026-05-07-cangjie-phase2-slice18-production-fixture-smoke-closure-review.md`

**Note:** Slice 18 发现的 dangling source edges 已在 Slice 19 中修复。Slice 19 采用 Synthetic Source Nodes 方案（非完整 constructor symbol extraction），通过在 graph emission 阶段为 reference source IDs emit synthetic callable source nodes 来修复 endpoint integrity。

**Phase 2 Slice 19 — Cangjie reference source endpoint integrity repair ✅ 完全成功（2026-05-07）：**
- 修复 Slice 18 暴露的 646 个 constructor call dangling edges（实际运行：125 unique dangling source IDs，770 dangling edges）
- Root cause 确认：Reference extraction 生成 `Constructor:<absolute-path>:<Owner>.init#arity` 格式 source IDs，但 Symbol extraction 未提取构造函数 symbols，导致 ID 策略不一致
- 采用方案：方案 B（Synthetic Source Nodes）——在 graph emission 阶段为 reference source IDs emit synthetic callable source nodes
- 实现内容：新增 `NodeKind::CallableSource` 枚举值 + `emit_synthetic_source_nodes()` 函数（~95 行代码变更）
- 新增 4 个 endpoint integrity regression tests：`test_no_dangling_source_ids`、`test_no_dangling_target_ids`、`test_endpoint_integrity_on_production_fixture`、`test_synthetic_nodes_are_marked`
- Before/After 数据：Nodes 715 → 1,361 (+646 synthetic)，Edges 3,401 → 3,401（unchanged），Dangling source IDs 125 → 0 (-100%)，Dangling target IDs 0 → 0（unchanged）
- 192/192 tests pass（without feature），263/263 pass（with feature，+4 new endpoint integrity tests）
- 不改 GitNexus-RC runtime/Tool/live repo
- Preflight：`docs/plans/2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-preflight.md`
- Execution Card：`docs/plans/2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-execution-card.md`
- Closure Review：`docs/plans/2026-05-07-cangjie-phase2-slice19-source-endpoint-integrity-closure-review.md`

**Phase 2 Slice 20 — Multi-project Cangjie production smoke ✅ 完全成功（2026-05-07）：**
- 对 4 个真实 Cangjie 项目运行 smoke test，验证 synthetic nodes 普适性
- Smoke targets：cangjie-GitNexus-Index/runtime/cjgui、cangjie/runtime/cjgui、CangjieSkills web_framework test、CangjieSkills json_parser test
- 统计数据：Total nodes=4,661、Total edges=10,847、Total synthetic nodes=2,064、Total duration=17.258s
- Endpoint integrity：✅ 所有 4 个 targets 都通过（dangling source=0，dangling target=0）
- Synthetic nodes 普适性：✅ 良好（synthetic nodes 比例 12%-47%，不同规模项目都有效）
- 输出确定性：✅ 所有 targets 两次运行结果一致
- Docs reconciliation：✅ 修正 docs/plans/README.md 中 "Constructor symbol extraction" 过期表述
- 实现内容：新增 `crates/cangjie/tests/multi_project_smoke.rs`（~250 行代码），含详细统计信息
- 192/192 tests pass（without feature），264/264 pass（with feature，+1 new multi-project smoke test）
- 不改 GitNexus-RC runtime/Tool/live repo
- Preflight：`docs/plans/2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-preflight.md`
- Execution Card：`docs/plans/2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-execution-card.md`
- Closure Review：`docs/plans/2026-05-07-cangjie-phase2-slice20-multi-project-production-smoke-closure-review.md`

**Phase 2 Slice 21 — Cangjie constructor symbol extraction ✅ 完全成功（2026-05-07）：**
- 语义核查结论：能安全推进，采用方案 C3（补充 + Fallback 共存）
- 新增 `CangjieSymbolKind::Init` 枚举变体 + `owner_name: Option<String>` 字段
- Init symbol extraction：tree-sitter query 捕获 class/struct body 中的 init 定义
- Constructor source ID → Init symbol node ID 映射：`resolve_source_id()` 函数
- Synthetic nodes coexistence policy：已被 init symbol 覆盖的 source ID 不再发 synthetic node
- Constructor 类 synthetic nodes 降为 0（fixture 验证）
- 新增 fixture `constructor-basic/` + `constructor-cross-file/`
- 新增 6 constructor integration tests + 7 endpoint integrity property tests
- 109 lib tests pass（with feature），95 pass（without feature），零新增依赖
- 不改 GitNexus-RC / Tool / live repo
- Preflight：`docs/plans/2026-05-07-cangjie-phase2-slice21-constructor-symbol-extraction-preflight.md`
- Execution Card：`docs/plans/2026-05-07-cangjie-phase2-slice21-constructor-symbol-extraction-execution-card.md`
- Closure Review：`docs/plans/2026-05-07-cangjie-phase2-slice21-constructor-symbol-extraction-closure-review.md`

**Phase 2 Slice 21 post-review follow-up — Init symbol node ID 唯一性修复 ✅ 完成（2026-05-07）：**
- 修复 Slice 21 中的 multi init duplicate graph node ID 问题
- 新增 `arity: Option<usize>` 字段到 `CangjieSymbol`，init 提取时计数参数
- `symbol_node_id()` 对 Init symbol 添加 `#arity` 后缀：`sym:<path>:Init:<Owner>.init#<arity>`
- `constructor_to_symbol_id` 映射键对齐 Constructor source ID 格式（含 `#arity`）
- 新增 6 个回归测试：duplicate ID 检测、MultiInit 唯一 ID、所有预期 init 存在、synthetic fallback 保留、endpoint integrity
- 287 tests pass（with feature），0 fail，零新增依赖
- 修复策略：arity-based 唯一 ID，与 Constructor source ID `#arity` 格式对齐
- 歧义处理：arity 不匹配时保留 synthetic fallback，不错误映射
- Closure Review：`docs/plans/2026-05-07-cangjie-phase2-slice21-init-node-id-uniqueness-followup-closure-review.md`

**Phase 2 production graph quality hardening — Function symbol node ID 唯一性修复 ✅ 完成（2026-05-08）：**
- 在 production fixture smoke 中发现 json_parser 项目有 16 个重复 Function node ID（toString ×6, asBool ×2, ...）
- 根因：symbol extraction 对 class methods 不提取 owner_name，所有同名方法产生相同 node ID
- 修复：Function symbol 提取 owner_name + arity，方法 name 格式化为 `Owner.funcName`，node ID 追加 `#arity`
- 新增 Method source_id → Function symbol node_id 映射（与 Constructor→Init 映射对齐）
- `extract_owner_name()` 新增 enumDefinition/interfaceDefinition 支持
- `count_init_params()` 重命名为 `count_params()`（通用化）
- `references.rs` `build_source_id()`: Function source ID 追加 `#arity`
- `resolve_source_id()`: 支持 `Method:` 前缀 source ID 映射
- Production smoke: 4/4 targets 全部 0 duplicate nodes, 0 dangling, deterministic
- 287+ tests pass（with feature），0 fail，零新增依赖
- Closure Review：`docs/plans/2026-05-08-cangjie-production-graph-quality-hardening-closure-review.md`

**Phase 2 import/reference quality hardening ✅ 完成（2026-05-08）：**
- ImportKind 枚举：ExplicitImport / WildcardImport / PackageAlias 三种 import 种类
- 差异化 confidence：ExplicitImport=0.85, PackageAlias=0.80, WildcardImport=0.70（原 flat 0.85）
- Disambiguation 重写：Unique ExplicitImport > Unique PackageAlias > 多 wildcard no-edge
- Dead code 清理：移除 SymbolConflict, detect_symbol_conflicts, calculate_wildcard_confidence, extract_package_from_path, calculate_specificity_score（~100 lines）
- Warning cleanup：package_name_from_target 添加 #[allow(dead_code)]，移除未使用 import 和变量
- 测试启用：2 个 #[ignore] 测试（exact match priority, ambiguous resolution）启用并通过
- Reason 字符串区分：`cross-file via explicit import` / `cross-file via package alias` / `cross-file via wildcard import`
- 核心原则："宁可 no-edge，也不要错误高置信度 edge"
- Production smoke: 4/4 targets 全部 0 duplicate, 0 dangling, deterministic
- 287+ tests pass（with feature），0 fail，零新增依赖
- Closure Review：`docs/plans/2026-05-08-cangjie-import-reference-quality-hardening-closure-review.md`

**Phase 2 post-hardening hygiene ✅ 完成（2026-05-08）：**
- 修复 references.rs resolve() 文档注释：更新为 ImportKind-based 策略描述
- 清理 no-feature warnings：references.rs 7 个 helper 添加 #[cfg(feature = "tree-sitter-cangjie")] gate
- graph.rs HashSet import gating, cangjie_inspect.rs predicates import gating
- Unit test module gating (references.rs tests now require tree-sitter-cangjie feature)
- Zero warnings in both no-feature and feature builds (excluding pre-existing scanner.c)
- Commit: `ba3db9f`

**Phase 2 Stage B — Function synthetic node elimination ✅ 完成（2026-05-08）：**
- Root cause: emit_cangjie_reference_edges() 缺少 function_to_symbol_id 映射
- Fix: 新增 function_to_symbol_id HashMap for Function symbols without owner_name
- resolve_source_id() 新增 Function: 前缀处理（精确匹配 + arity fallback）
- Before/After: Function synthetic 1508 → 0（all 4 production targets）
- Total nodes: 4919 → 3411（不再需要 CallableSource synthetic nodes）
- All synthetic nodes (Constructor/Method/Function) = 0 across 4 targets
- 0 duplicate node IDs, 0 duplicate edge triples, 0 dangling, deterministic
- 3 new unit tests + all existing tests pass
- 不删除 synthetic fallback（unresolved source IDs 仍可 fallback）
- Preflight: `docs/plans/2026-05-08-cangjie-stage-b-function-synthetic-mapping-preflight.md`
- Closure Review: `docs/plans/2026-05-08-cangjie-stage-b-function-synthetic-mapping-closure-review.md`

**Phase 2 Slices 18+（后续）：**
- ~~Slice 13：function call reference extraction~~ ✅ 完成
- ~~Slice 14a：wildcard import expansion~~ ✅ 完成
- ~~Slice 14b：alias resolution~~ ✅ 完成
- ~~Slice 15：wildcard import edge quality~~ ✅ 完成
- ~~Slice 16：Cangjie graph output parity smoke~~ ✅ 完成
- Slice 17+：后续 bounded slices（需 preflight）
- LSP client（P1 future，触发 stop-line，需先写 preflight）

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
5. ~~Phase 2 Slice 4 — tree-sitter Cangjie vendor gate~~ ✅ 完成（docs-only）
6. ~~Phase 2 Slice 5 — tree-sitter Cangjie 集成~~ ✅ 完成
7. ~~Phase 2 Slice 6 — tree-sitter Cangjie AST symbol extraction~~ ✅ 完成
8. ~~Phase 2 Slice 7 — Cangjie graph output~~ ✅ 完成
9. ~~Phase 2 Slice 8 — Cangjie diagnostics runner~~ ✅ 完成
10. ~~Phase 2 Slice 9 — diagnostics integration~~ ✅ 完成
11. ~~Phase 2 Slice 10 — same-file reference extraction~~ ✅ 完成
12. ~~Phase 2 Slice 11 — import resolution + IMPORTS edges~~ ✅ 完成
13. ~~Phase 2 Slice 11b — cjpm tree + external dep resolution~~ ✅ 完成
14. ~~Phase 2 Slice 12 — cross-file reference extraction~~ ✅ 完成
15. ~~Phase 2 Slice 13 — function call reference extraction~~ ✅ 完成
16. ~~Phase 2 Slice 14a — wildcard import expansion~~ ✅ 完成
17. ~~Phase 2 Slice 14b — alias resolution~~ ✅ 完成
18. ~~Phase 2 Slice 15 — wildcard import edge quality~~ ✅ 完成
19. ~~Phase 2 Slice 16 — Cangjie graph output parity smoke~~ ✅ 完成
20. ~~Phase 2 Slice 17 — Cangjie CLI surface MVP~~ ✅ 完成
21. ~~Phase 2 Slice 18 — Cangjie production fixture smoke~~ ✅ 完成
22. ~~Phase 2 Slice 19 — Cangjie reference source endpoint integrity repair~~ ✅ 完成
23. ~~Phase 2 Slice 20 — Multi-project Cangjie production smoke~~ ✅ 完成
24. ~~Phase 2 Slice 20 follow-up — multi-project smoke opt-in hygiene~~ ✅ 完成（#[ignore] + 文档闭合）
25. ~~Phase 2 Slice 21 — Cangjie constructor symbol extraction~~ ✅ 完成（Init kind + source ID 映射 + synthetic coexistence）
26. ~~Phase 2 Slice 21 post-review follow-up — Init symbol node ID 唯一性修复~~ ✅ 完成（arity-based unique ID）
27. ~~Phase 2 production graph quality hardening — Function symbol node ID 唯一性修复~~ ✅ 完成（owner + arity）
28. ~~Phase 2 import/reference quality hardening~~ ✅ 完成（ImportKind + confidence + disambiguation + dead code cleanup）
29. ~~Phase 2 post-hardening hygiene~~ ✅ 完成（comments + no-feature warnings cleanup）
30. ~~Phase 2 Stage B — Function synthetic node elimination~~ ✅ 完成（function_to_symbol_id mapping）
31. **Production Acceptance Stage 1 — Audit + Preflight** ✅ 完成（2026-05-08）：
   - 写 production acceptance preflight：CLI output contract, quality gate coverage, gaps, stop-lines
   - 判定：READY for local trial use as development-quality graph tool
   - Preflight: `docs/plans/2026-05-08-cangjie-production-acceptance-preflight.md`
32. **Production Acceptance Stage 2 — Harden Test/Smoke Ergonomics** ✅ 完成（2026-05-08）：
   - 重写 `multi_project_smoke.rs`：区分 Fixture（always available, hard assert）vs Production（#[ignore] guarded, graceful skip）
   - 新增 3 fixture-based smoke tests（imports-basic, constructor-basic, reference-cross-file-basic）
   - 改进输出格式（PASS/FAIL/SKIP 表格 + summary）
   - 修复 fixture_path 解析（CARGO_MANIFEST_DIR → .parent() ×2）
33. **Production Acceptance Stage 3 — Contract Snapshot / Regression Guard** ✅ 完成（2026-05-08）：
   - 新增 `graph_contract.rs`：16 tests on 3 fixtures
   - 验证 node/edge kind sets, known symbol IDs, known edge triples, quality gates
   - 无 JSON snapshots，无 sort-order binding
   - Closure Review: `docs/plans/2026-05-08-cangjie-contract-regression-guard-closure-review.md`
34. **Production Acceptance Addendum — Portable Smoke Fixture** ✅ 完成（2026-05-08）：
   - 新增 `fixtures/cangjie/portable-smoke/`：3-file Cangjie project exercising all major extraction paths
   - 覆盖所有 Symbol kinds：Function, Class, Struct, Enum, Interface, TypeAlias, Init（含 #arity）
   - 覆盖所有 Edge kinds：ContainsPackage, OwnsSource, Defines, Uses, Imports
   - 跨文件 + 同文件 Uses edges，Constructor/Function call references
   - 新增 `fixture_smoke_portable()` in multi_project_smoke.rs（4 fixture tests total）
   - 新增 8 contract tests in graph_contract.rs（24 tests total on 4 fixtures）
   - 27 nodes, 36 edges, 0 synthetic, 0 duplicate, 0 dangling, deterministic
   - 192/192 no-feature pass, 284+/284+ feature pass, 4/4 production smoke pass
   - 独立 QUALITY.md 提取完成（repo root，acceptance criteria single source of truth）
   - Strict flag follow-up ✅ 完成：8 dedicated CLI tests + QUALITY.md --strict section + closure review
35. **Rust Production Readiness Smoke Audit + CALLS Endpoint Integrity Fix** ✅ 完成（2026-05-08）：
	   - 对 gitnexus-rust-core 自身 + 2 fixture 做 read-only smoke
	   - 发现 459 dangling CALLS edges（全部指向外部 std::* 符号）
	   - 根因：graph.rs emit CALLS edges for all resolved calls，但外部 symbol node 从未被提取
	   - 修复：graph.rs 新增外部 symbol node 补全逻辑（+43 minimal external symbol nodes, isExternal=true）
	   - Dangling CALLS target: 459 → 0，CALLS edges 全部保留
	   - 更新 c10 test：新增 endpoint integrity + isExternal 验证
	   - 全部测试通过（cangjie_inspect 18/18, project_model_graph_emit 10/10, production smoke 4/4）
	   - Preflight: `docs/plans/2026-05-08-rust-production-readiness-preflight.md`
	   - Closure Review: `docs/plans/2026-05-08-rust-production-readiness-closure-review.md`
	36. **Rust Graph Contract Tests** ✅ 完成（2026-05-08）：
	   - 仿照 Cangjie graph_contract.rs 模式，创建 Rust graph contract regression tests
	   - 新建 `fixtures/rust/portable-smoke/`：3 文件 Rust project，覆盖所有核心节点/边类型
	   - 新建 `crates/cli/tests/project_model_graph_contract.rs`：8 tests（quality gates, node/edge kinds, known symbols/edges, endpoint integrity）
	   - Graph 产出：16 nodes, 25 edges, 0 dup, 0 dangling, 确定性输出
	   - 全部测试通过（cangjie_inspect 18/18, project_model_graph_emit 10/10, project_model_graph_contract 8/8）
	   - Closure Review: `docs/plans/2026-05-08-rust-graph-contract-closure-review.md`
	37. **Rust Enum Constructor Resolution** ✅ 完成（2026-05-08）：
		   - 将 stdlib enum variant constructor（Some/Ok/Err/None）从过滤改为解析
		   - 新增 `CallKnownEnumConstructor` reason + `resolve_known_enum_constructor()` 函数
		   - 改进：+305 resolved calls，resolution rate 53.7% → 62.4%（+8.7pp）
		   - 更新 call-enum-filter + c11-receiver-type golden fixtures
		   - 全部测试通过（cangjie_inspect 18/18, graph_contract 24/24, project_model_graph_contract 8/8, call comparison 19/19 fixtures pass）
		   - Commit: `d9f5997`
		   - Closure Review: `docs/plans/2026-05-08-rust-enum-constructor-resolution-closure-review.md`
39. **Cangjie Production Acceptance 固化** ✅ 完成（2026-05-08）：
   - 基线全量回归验证：no-feature ~200 pass, feature ~330 pass
   - Production smoke: 4/4 targets PASS（synth=0, dup=0, dang=(0,0), det=true）
   - 文档同步：README.md + AGENTS.md + plans README 过期 stats 更新
   - 替换 plans README 过期"Slice 7 recommended next"为当前状态总结
   - Commit: `e81fe19`
   - Closure Review: `docs/plans/2026-05-08-cangjie-production-acceptance-consolidation-closure-review.md`
40. **Phase 2d — let-binding 构造函数链 receiver type 推断** ✅ 完成（2026-05-08）：
   - 扩展 scan_variable_type_annotation 支持通过 RHS 已知构造函数推断变量类型
   - 新增 KNOWN_CONSTRUCTORS 表（10 个构造函数 → 6 种基础类型）+ lookup_constructor_type
   - Phase 2d 逻辑：在无类型注解时扫描 `let v = Constructor(...)` → 推断 receiver type
   - 恢复被误删除的 strip_generics 函数（calls.rs 中 4 处调用）
   - 新增 c12-let-constructor-method fixture（14 calls, 全部解析）
   - Improvement: +74 resolved calls, receiver-type-method-resolved 164 → 235
   - Resolution rate: 62.2% → 64.1%（+1.9pp），2252/3514
   - 全部测试通过（call comparison 20/20 fixtures, graph_contract 24/24, cangjie_inspect 18/18）
   - Commit: `3898fb7`
   - Closure Review: `docs/plans/2026-05-08-rust-phase2d-constructor-chain-closure-review.md`
40b. **STDLIB_TYPE_METHODS 扩展 — PathBuf, HashSet, BTreeMap** ✅ 完成（2026-05-08）：
   - 新增 3 种 stdlib 类型的 method 条目以完成 KNOWN_CONSTRUCTORS 覆盖
   - HashSet: 7 methods, BTreeMap: 7 methods, PathBuf: 13 methods
   - Improvement: +31 resolved calls, receiver-type-method-resolved 235 → 266
   - Resolution rate: 64.1% → 65.0%（+0.9pp），2283/3514
   - 本轮合计：62.2% → 65.0%（+105 resolved calls, +2.8pp）
   - Commit: `6d0f157`
41. Priority 2/4/5 — 后续 Rust/Cangjie bounded slices（需 preflight）
42. **Phase 2e — 跨文件 same-crate call resolution** ✅ 完成（2026-05-08）：
   - 新增 CalleeIndex 跨文件 same-crate 搜索（`lookup_crate_wide_function` / `lookup_crate_wide_type`）
   - 新增 `CallSameCrateResolved` reason（confidence 0.80）
   - `resolve_free_function` 新增 step 2.5：crate-wide unique function search
   - `resolve_type_module` 新增 step 3：crate-wide type search（辅助 associated function）
   - `resolve_qualified_path` 新增 CalleeIndex fallback（模块链解析失败时）
   - 新增 c13-cross-file-same-crate fixture（compile-valid, 2 calls resolved）
   - 新增 source_to_package 映射到 CalleeIndex
   - Improvement: +38 resolved calls, 65.0% → 65.6%（2321/3539）
   - 18 calls.rs → stdlib_tables.rs 跨文件调用全部解析
   - 全部测试通过（call comparison 21/21 fixtures, graph_contract 24/24, cangjie_inspect 18/18）
   - Commits: `55bc86a`, `669ddc6`
   - Closure Review: `docs/plans/2026-05-08-rust-cross-file-same-crate-closure-review.md`
	43. **Rust Graph Contract Expansion** ✅ 完成（2026-05-08）：
	   - 新增 2 个 Rust graph contract fixtures：`imports-cross-crate`（外部 symbol node + ACCESSES）和 `multi-module`（跨文件 crate:: 路径 CALLS）
	   - 新增 15 个 contract tests → 总计 23 tests on 3 fixtures（Before: 8 tests on 1 fixture）
	   - imports-cross-crate：14 nodes, 22 edges, 8 edge types, 4 external symbol nodes, 7 CALLS（含 external crate）
	   - multi-module：10 nodes, 12 edges, 5 edge types, 2 source files, 3 CALLS（含 crate:: 路径）
	   - 缩小与 Cangjie graph contract（24 tests on 4 fixtures）的覆盖差距
	   - 全部测试通过（cangjie_inspect 18/18, graph_contract 24/24, project_model_graph_contract 23/23）
	   - Closure Review: `docs/plans/2026-05-08-rust-graph-contract-expansion-closure-review.md`
	44. **Phase 2f — wildcard import 源模块感知消歧** ✅ 完成（2026-05-08）：
	   - 新增 `build_wildcard_module_map()` + CalleeIndex.wildcard_modules 字段
	   - `resolve_free_function` 新增 step 2.5b：wildcard import 源模块感知消歧
	   - 规范化 wildcard import original_path（裸名称 → crate::module 路径，含 :: 路径直接去 ::*）
	   - 新增 c14-wildcard-disambiguation fixture（compile-valid, 2 calls）
	   - Improvement: +5 resolved calls（split_last_segment all resolved），65.6% → 65.7%（2338/3557）
	   - call-same-crate-resolved: 18 → 23
	   - 全部测试通过（call comparison 22/22 fixtures, graph_contract 24/24, cangjie_inspect 18/18）
	   - Closure Review: `docs/plans/2026-05-08-rust-wildcard-import-disambiguation-closure-review.md`
	45. **Docs Consolidation — Rust Quality Gates + Stale Stats Fix** ✅ 完成（2026-05-08）：
	   - QUALITY.md 新增完整「Rust Graph Quality Gates」章节（质量门、合同回归门、合同 fixture 表、生产统计、已知差距、stop-lines）
	   - README.md 过期 stats 修复：resolution rate 65.6% → 65.7%，call fixtures 15→22，新增 2 个 call forms
	   - docs/plans/README.md 推荐下一篇计划更新为当前准确 openings
	   - 全部测试通过（no-feature + feature），cargo fmt + git diff clean
