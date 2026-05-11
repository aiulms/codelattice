# CodeLattice Plans Index

最后更新：2026-05-11（Runtime isolation: AI IDE MCP points to CodeLattice-Tool stable runtime）

## 用途

本目录存放 CodeLattice 的计划文档（preflight / execution card / closure review）。

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

**Rust 线：** Resolution rate 65.7%（2370/3609）。0 dangling CALLS edges。Graph contract 58/58（8 fixtures）。Call comparison 24/24 fixtures。Symbol comparison 4/4 fixtures。Graph 产出：1524 nodes, 2438 edges, 1054 CALLS edges。Enum variant 提取 + 分类修复已完成（Slice 53）：173 variant symbols，318 enum-constructor calls 全量 resolved。第 7-8 个 fixture enum-variant + workspace-member 已完成（Slice 54 + 55）。method-calls 仍为主要 gap（~1204 unresolved，stop-line: no type inference）。Unresolved free-function 15 个全部为局部闭包/cfg-gated/跨模块 variant（均在 stop-line 后）。

**Productization 线（本轮新增，2026-05-09）：**
- ✅ Unified CLI Surface：analyze / quality / summary 三个产品化命令，--language auto 检测
- ✅ Unified Output Contract：docs/architecture/unified-output-contract.md（GraphSummary + QualityGateResult + LanguageAnalysisResult）
- ✅ Quality Command：JSON 输出 + exit codes 0/1/2，7 个质量门
- ✅ Bridge Adapter：crates/cli/src/bridge_format.rs（Rust/Cangjie → GitNexus-RC 兼容格式）
- ✅ Smoke Targets Config：docs/smoke-targets-config.md（16 targets: 12 Tier 1 + 5 Tier 2）
- ✅ Bridge Preflight：docs/architecture/bridge-preflight.md（差异矩阵 + stop-line）
- ✅ 15 integration tests + 7 bridge unit tests
- ✅ Productization Closure Review：docs/plans/2026-05-09-productization-phase-closure-review.md
- ✅ **Local Trial Packaging**（2026-05-09）：scripts/build.sh + scripts/smoke.sh，一键构建 + 快速验证
- ✅ **Analyze --strict Flag**（2026-05-09）：analyze 命令新增 --strict flag，质量门失败时 exit non-zero，与 Cangjie inspect --strict 行为对齐
- ✅ **Cross-repo Consumer Dry-run**（2026-05-09）：GitNexus-RC 消费侧只读审计（16 文件），Bridge 兼容性报告 v1.4.0（三次审计确认 §8.6，bridge adapter 全部 4 文件细节），2 个 bridge adapter 修复（symbol kind + edge confidence），bridge_roundtrip 26 tests（13 Rust + 13 Cangjie）+ deterministic 修复（strip generatedAt）
- ✅ **Bridge adapter 分离**（2026-05-09）：bridge_format.rs（~890 行）拆分为 rust_bridge.rs + cangjie_bridge.rs + bridge_format.rs（共享类型 + 边分组），零行为变化，纯结构重构
- ✅ **Consumer Contract 固化**（2026-05-09）：新增 §零 三级字段分类（Stable/Adapter-Required/Intentionally-Unstable）、§五 Rust vs Cangjie 差异表、§六 Node ID 不稳定边界
- ✅ **Adapter Readiness Test Pack**（2026-05-09）：bridge_roundtrip 22→26 tests，新增 symbol kind 白名单 + packageId 交叉引用验证
- ✅ **Local Trial Packaging**（2026-05-09）：新增 scripts/verify-bridge.sh 面向 RC adapter 开发者，build.sh 增加 bridge format 示例，README.md 增加 bridge 验证章节
- ✅ **GitNexus-RC Adapter Preflight**（2026-05-09，v1.1.0）：docs/plans/2026-05-09-gitnexus-rc-adapter-preflight.md — 已更新为落地后状态：bridge adapter 4 文件 ~921 行（超出预研 ~320 行）、转换边界、风险、11 项验收清单、下一步待做（Tool propagation / 端到端验证 / USES 渲染样式）
- ✅ **GitNexus-RC Bridge Adapter landed（cross-repo 状态）**：过渡消费端已落地 `rust-core-bridge-adapter/`（GitNexus-RC commit `26a21b5e`，closure `75107091`）。Rust-core 侧仍把该路径视为 legacy compatibility consumer，不作为本项目长期产品身份。

- ✅ **Public Identity Cleanup**（2026-05-09）：README.md / build.sh / verify-bridge.sh 中 GitNexus-RC 特定引用替换为中性表述（"下游消费格式"/"下游消费方"），保持 CLI flag `--format gitnexus-rc` 不变
- ✅ **Production Trial Acceptance Checklist**（2026-05-09）：docs/plans/2026-05-09-production-trial-acceptance-checklist.md — alpha production trial 验收清单，11 节全覆盖（命令/字段/smoke/质量门/AI 消费接口/已知限制/前置条件）

- ✅ **Alpha Trial Bridge Endpoint + Stdout Purity Closure**（2026-05-09）：[`2026-05-09-alpha-trial-bridge-endpoint-stdout-purity-closure-review.md`](2026-05-09-alpha-trial-bridge-endpoint-stdout-purity-closure-review.md) — 修复 Rust workspace dangling edges（workspace→package, diagnostic→symbol 映射）、Cangjie stdout purity（scanner.c fprintf→stderr, main.rs cfg guard）。Rust 自身 bridge JSON 0 dangling → Tool 导入成功（4711 nodes/7000 edges）。Cangjie cjgui bridge JSON stdout 纯净 → Tool 导入成功（4851 nodes/7000 edges）。全量回归通过。**结论：Alpha Production Trial Ready。**

- ✅ **Alpha Production Trial Runbook**（2026-05-09）：[`2026-05-09-alpha-production-trial-runbook.md`](2026-05-09-alpha-production-trial-runbook.md) — 操作手册：适用/不适用范围、标准命令（Rust/Cangjie bridge 生成 + Tool 导入）、成功/失败判定、回滚/清理流程、风险边界、执行 AI 最小 checklist。Explicit opt-in，不替代 TS adapter。

- ✅ **Alpha Trial 端到端 Smoke 脚本**（2026-05-09）：`scripts/alpha-trial-smoke.sh` — 验证 Rust/Cangjie bridge JSON → Tool `--experimental-rust-core-bridge-graph` 导入全链路。使用 portable-smoke fixture，不写 live repo，不自动 commit。支持 `--rust-only` / `--cangjie-only`。

- ✅ **Legacy Naming Cleanup Phase 1**（2026-05-09）：[`2026-05-09-public-identity-and-legacy-command-cleanup-plan.md`](2026-05-09-public-identity-and-legacy-command-cleanup-plan.md) — Phase 1 审查完成：README.md / scripts 已中性化，0 处 npx gitnexus 生产命令，残留 ~109 处 `GitNexus-RC` 均为桥接适配器接口事实描述或历史文档。runbook 适用范围措辞小幅修正。Phase 2/3 暂不执行。

- ✅ **Alpha Trial Maintenance and Failure Playbook**（2026-05-09）：[`2026-05-09-alpha-trial-maintenance-and-failure-playbook.md`](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md) — 维护手册：日常/周期 smoke 推荐、7 类失败分类（stdout purity / dangling / duplicate / deterministic drift / adapter validation / header artifact / command authority）及第一响应动作、试用期记录格式、退出 alpha / 升级 beta 候选条件。

- ✅ **f97f733 复核通过**（2026-05-09）：复核结论 — commit 无误清兼容名、无误删 bridge compatibility 文档、`--format gitnexus-rc` 和 `--experimental-rust-core-bridge-graph` 均保留、无 `npx gitnexus` 引入、无 generatedAt "值稳定" 描述、无 sed-as-fix 建议。旧名残留扫描确认：0 处必须修复项。

- 📝 **Periodic Alpha Trial Log Template**（2026-05-09）：[`2026-05-09-periodic-alpha-trial-log-template.md`](2026-05-09-periodic-alpha-trial-log-template.md) — 空白试用记录模板（不包含伪造数据）。包含：date / executor / target / command / bridge JSON size / stdout purity / Tool ingestion / stats / quality checks / cleanup / failure classification / rollback / final status / notes。

- 📝 **Beta Readiness Criteria Preflight**（2026-05-09）：[`2026-05-09-beta-readiness-criteria-preflight.md`](2026-05-09-beta-readiness-criteria-preflight.md) — Alpha → Beta 升级条件：≥ 5 次真实项目 trial + ≥ 3 周无回归 + ≥ 3 条 trial log + 外部 AI 独立执行。Beta 不包含默认引擎切换 / WebUI / MCP / 多语言扩张。Go/No-Go checklist 12 项。

- ✅ **Periodic Alpha Trial Run #001**（2026-05-09）：[`2026-05-09-periodic-alpha-trial-run-001.md`](2026-05-09-periodic-alpha-trial-run-001.md) — 第一次真实项目 periodic trial。Rust self-analysis（1700 nodes, 2634 edges, 0 dangling）+ Cangjie cjgui（903 nodes, 3252 edges, 0 dangling）。两个 target 全部 PASS，Tool 导入成功，stdout purity 通过。

- 📝 **Beta Go/No-Go Review #001**（2026-05-09）：[`2026-05-09-beta-readiness-go-no-go-review-001.md`](2026-05-09-beta-readiness-go-no-go-review-001.md) — 第一次 Beta 草评。8 项 criteria：2 PASS + 3 PARTIAL + 3 NOT YET ENOUGH DATA。结论：Alpha 维持，Beta Not yet，需 ≥ 3 轮 trial 积累。无 blocker。

- ✅ **Alpha Smoke Reliability Fix**（2026-05-10）：`scripts/alpha-trial-smoke.sh` — 不再用 `tool | grep -q` 管道判断成功；改为捕获输出到临时文件 + exit code 判断 + 文本确认（容错）；统一 `tool_bridge_import()` helper；修复 NODE_BIN 默认路径。Smoke 验证 8/8 PASS。

- ✅ **Periodic Alpha Trial Run #002**（2026-05-10）：[`2026-05-10-periodic-alpha-trial-run-002.md`](2026-05-10-periodic-alpha-trial-run-002.md) — 第二次真实项目 periodic trial。Rust self-analysis（1700 nodes, 2634 edges）+ Cangjie cjgui（903 nodes, 3252 edges）。两个 target 全部 PASS，graph stats 与 Run #001 完全一致，零回归。

- 📝 **Beta Readiness Evidence Board**（2026-05-10）：[`2026-05-10-beta-readiness-evidence-board.md`](2026-05-10-beta-readiness-evidence-board.md) — Beta criteria 进度追踪表（living document）。Run #001/#002/#004 PASS；Run #003 external AI attempted but not counted。External AI independent run 已 PASS，Beta 仍 NOT YET。

- 📝 **External AI Run #003 Task Package**（2026-05-10）：[`2026-05-10-external-ai-periodic-alpha-trial-run-003-task-package.md`](2026-05-10-external-ai-periodic-alpha-trial-run-003-task-package.md) — 自包含外部 AI 独立执行任务包（10 节），含完整命令、workspace 路径、成功标准、禁止操作、trial log 模板。不依赖 chat context，直接交付执行。

- ❌ **Periodic Alpha Trial Run #003**（2026-05-10）：[`2026-05-10-periodic-alpha-trial-run-003.md`](2026-05-10-periodic-alpha-trial-run-003.md) — 外部 AI 独立执行。Rust/Cangjie bridge trial 均 PASS（stdout purity、0 dangling、0 duplicate、deterministic、Tool ingestion、cleanup），但 baseline `cargo fmt --check` 失败，因此整体 FAIL / 不计入 Beta。

- 📝 **Beta Go/No-Go Review #002**（2026-05-10）：[`2026-05-10-beta-readiness-go-no-go-review-002.md`](2026-05-10-beta-readiness-go-no-go-review-002.md) — 第二次 Beta 草评。8 项 criteria：2 PASS + 3 PARTIAL + 3 NOT YET ENOUGH DATA。结论：Alpha 继续健康，Beta Not yet，需外部 AI 执行 + ≥ 3 周日历跨度。无 blocker。

- 📝 **Beta Go/No-Go Review #003**（2026-05-10）：[`2026-05-10-beta-readiness-go-no-go-review-003.md`](2026-05-10-beta-readiness-go-no-go-review-003.md) — 第三次 Beta 草评。Run #003 为 external AI attempted-not-PASS；新增 blocker 为 pre-existing `cargo fmt --check` drift。结论：Beta NO-GO，Alpha 继续 ACTIVE。

- ✅ **Rust CALLS Confidence/Reason Quality Hardening**（2026-05-10）：[`preflight`](2026-05-10-rust-calls-confidence-reason-quality-preflight.md) / [`closure`](2026-05-10-rust-calls-confidence-reason-quality-closure.md) — 固化 19 种 Rust call form 的 confidence/reason 矩阵。24 个 fixture 全部有 expected-calls.json 自动验证。新增 [`rust-calls-confidence-matrix.md`](../architecture/rust-calls-confidence-matrix.md) 策略参考文档。无代码修复，无 behavior change。

- ✅ **Cangjie Reference Edge Quality Hardening**（2026-05-10）：[`preflight`](2026-05-10-cangjie-reference-edge-quality-preflight.md) / [`closure`](2026-05-10-cangjie-reference-edge-quality-closure.md) — 固化 Cangjie 7 种 same-file edge + 3 种 cross-file import confidence 层级。新增 11 个 confidence/reason/ambiguous 测试（alias_reference +4, cross_file_import_confidence +7）。无 behavior change。

- ✅ **Path Portability After Rename**（2026-05-10）：[`closure`](2026-05-10-path-portability-after-codelattice-rename-closure.md) — `cargo clean` 清除 stale `env!("CARGO_MANIFEST_DIR")` 缓存。13 个因旧路径失败的测试全部恢复 PASS。源代码路径推导逻辑正确（`CARGO_MANIFEST_DIR` + parent），无需代码修改。

- ✅ **Rust Method/Associated Call + Cangjie Constructor/Interface Dual-Line Quality Enhancement**（2026-05-10）：[`Rust preflight`](2026-05-10-rust-method-associated-call-preflight.md) / [`Cangjie preflight`](2026-05-10-cangjie-constructor-interface-call-preflight.md) / [`closure`](2026-05-10-dual-line-language-quality-closure-review.md) — Rust: 修正 c11 stale comment（函数参数类型注解已支持），新增 4 个 confidence/reason 回归测试（receiver type / constructor chain / associated fn / disambiguation）。Cangjie: 新增 2 个 constructor call 设计合同回归测试（call→Class symbol / Init→SourceFile Defines）。无 runtime 变更，质量天花板已达到。

- ✅ **Run #003 Format Hygiene Cleanup**（2026-05-10）：[`closure`](2026-05-10-run003-format-hygiene-cleanup-closure.md) — 修复 `cargo fmt --check` drift（2 个 test 文件）。`cargo fmt` applied，仅格式变化。Run #003 保持 FAIL / not counted。Blocker 已清除，后续应执行 External AI Run #004。

- ✅ **Periodic Alpha Trial Run #004**（2026-05-10）：[`2026-05-10-periodic-alpha-trial-run-004.md`](2026-05-10-periodic-alpha-trial-run-004.md) — 外部 AI 独立 retry。Mandatory gates 全 PASS（含 `cargo fmt --check`、bridge_roundtrip、alpha smoke）。Rust/Cangjie bridge trial 均 PASS，Tool ingestion 成功，0 dangling / 0 duplicate，registry cleanup 完成。计入 Beta。

- 📝 **Beta Go/No-Go Review #004**（2026-05-10）：[`2026-05-10-beta-readiness-go-no-go-review-004.md`](2026-05-10-beta-readiness-go-no-go-review-004.md) — 第四次 Beta 草评。External AI criterion 升级为 PASS；技术 blocker none；Beta 仍 NOT YET，剩余 trial count + calendar span。

- 📝 **Local Default Replacement Preflight**（2026-05-10）：[`2026-05-10-local-default-replacement-preflight.md`](2026-05-10-local-default-replacement-preflight.md) — 本机默认替换调查，仅文档，不启用。结论：推荐下一步在用户批准后实现 language-aware switch script；Rust/Cangjie analyze generation 可优先 CodeLattice，GitNexus-RC 必须继续作为 Tool/MCP/WebUI/query/refactor/fallback。

- ✅ **MCP v0 Thin stdio Wrapper**（2026-05-10）：[`preflight`](2026-05-10-mcp-v0-thin-wrapper-preflight.md) / [`contract`](../architecture/mcp-v0-contract.md) / [`closure`](2026-05-10-mcp-v0-thin-wrapper-closure.md) — 新增 MCP stdio server（`gitnexus-rust-core-cli mcp`），4 个工具（analyze/quality/summary/smoke），subprocess-based thin wrapper，无新增依赖，10 个集成测试全部通过。Read-only，path deny list，timeout 保护。

- ✅ **MCP v0.1 Practical AI Layer**（2026-05-10）：[`preflight`](2026-05-10-mcp-v0-1-practical-ai-layer-preflight.md) / [`dogfood`](2026-05-10-mcp-v0-1-dogfood-report.md) / [`closure`](2026-05-10-mcp-v0-1-practical-ai-layer-closure.md) — 新增 4 个 AI 查询工具（graph_overview / unresolved_report / symbol_search / export_bridge），output shaping（compact analyze / failed gates first / smoke hints），统一错误结构（code/message/details/hint），dogfood harness（8/8 pass），18 个集成测试全部通过。无新增依赖。

- ✅ **MCP v0.2 Local Graph Intelligence**（2026-05-10）：[`closure`](2026-05-10-mcp-v0-2-local-graph-intelligence-closure.md) — 新增 8 个本地图谱智能工具（symbol_context / calls_from / calls_to / impact_preview / query_graph / project_overview / repo_registry / rename_preview），共享 GraphView 层，BFS 遍历，27 个集成测试，17/17 dogfood pass。无新增依赖。

- ✅ **MCP Local Client Integration**（2026-05-10）：[`smoke report`](2026-05-10-mcp-local-client-integration-smoke-report.md) — Sidecar startup wrapper（`scripts/codelattice-mcp.sh`）、Client 配置示例（Codex/Claude/opencode，[`setup doc`](../architecture/mcp-local-client-setup.md)）、Local client smoke 7/7 pass、工具体验评估、v0.3 优先级建议。不切默认，不替代 GitNexus MCP。

**Public Identity / Rename 线（Active，2026-05-09）：**
- ✅ **CodeLattice Local Path + Index Refresh**：本地目录从 `/Users/jiangxuanyang/Desktop/gitnexus-rust-core` 改为 `/Users/jiangxuanyang/Desktop/codelattice`；GitCode remote 改为 `https://gitcode.com/aiulms/codelattice.git`；Tool index 已刷新为 repo `codelattice`（4104 symbols / 7170 relationships / 157 flows）。旧名 `gitnexus-rust-core` 仅作为历史事实、兼容 binary/package/flag 名保留。

- ✅ **CodeLattice Rename Follow-up Closure**（2026-05-09）：[`2026-05-09-codelattice-rename-followup-closure.md`](2026-05-09-codelattice-rename-followup-closure.md) — rename 后残留扫描：修复 trial run-001 和 beta review-001 中的旧路径/旧名（4 处）；151 处旧名残留分类为兼容保留 / 历史事实 / future cleanup；Tool index 确认为 codelattice；alpha-trial-smoke 副作用已控制。
- 📝 **Product Positioning and Rename Preflight Draft**：[`2026-05-09-product-positioning-and-rename-preflight-draft.md`](2026-05-09-product-positioning-and-rename-preflight-draft.md)
- 结论初稿：技术底座已成形，产品身份尚未成形；下一刀建议先做 public-facing identity cleanup，而不是直接最终改名。
- 本 draft 不冻结最终命名、不改 runtime、不改 CLI/schema，只用于讨论完成度、可改动性和工程量。
- **Production Trial Readiness and Roadmap Pivot Preflight**：[`2026-05-09-production-trial-readiness-and-roadmap-pivot-preflight.md`](2026-05-09-production-trial-readiness-and-roadmap-pivot-preflight.md)
- 结论：路线不再以复刻某个现有工具为核心叙事；短期收束为 Rust/Cangjie 本地代码上下文核心的 alpha production trial。
- 第一版 production trial 不要求 UI/Web/MCP/多语言大覆盖，优先冻结命令、输出字段、quality gates、真实项目 smoke 和 AI 最小消费接口。
- **Legacy Naming Compatibility Cleanup Preflight**：[`2026-05-09-legacy-naming-compatibility-cleanup-preflight.md`](2026-05-09-legacy-naming-compatibility-cleanup-preflight.md)
- 结论：旧名清理分两层推进；先清 public-facing 叙事，runtime/API/CLI 旧名后续以 compatibility alias 方式迁移，不能一刀切。

## 当前推荐下一篇计划

**✅ Alpha Production Trial ACTIVE — Run #001/#002/#004 PASS; Run #003 attempted but FAIL / not counted. External AI independent run PASS. Beta NOT YET. Local default replacement remains preflight-only and is NOT enabled.**

技术验证和操作规程均已固化。Run #004 外部 AI 独立 retry 已通过并计入 Beta：
- [Trial Run #001](2026-05-09-periodic-alpha-trial-run-001.md) — Rust + Cangjie 双 target PASS
- [Trial Run #002](2026-05-10-periodic-alpha-trial-run-002.md) — Rust + Cangjie 双 target PASS，graph stats 与 Run #001 完全一致
- [Trial Run #003](2026-05-10-periodic-alpha-trial-run-003.md) — External AI independent run attempted；Rust/Cangjie bridge 子项 PASS，但 `cargo fmt --check` baseline FAIL，整体不计入 Beta
- [Trial Run #004](2026-05-10-periodic-alpha-trial-run-004.md) — External AI independent retry PASS；mandatory gates + Rust/Cangjie bridge trial 全部通过，计入 Beta
- [Beta Go/No-Go Review #001](2026-05-09-beta-readiness-go-no-go-review-001.md) — Not yet Beta, 证据积累中
- [Beta Go/No-Go Review #002](2026-05-10-beta-readiness-go-no-go-review-002.md) — Alpha continues healthy, Beta Not yet
- [Beta Go/No-Go Review #003](2026-05-10-beta-readiness-go-no-go-review-003.md) — NO-GO for Beta due Run #003 baseline failure + insufficient trial count/calendar span
- [Beta Go/No-Go Review #004](2026-05-10-beta-readiness-go-no-go-review-004.md) — NO-GO for Beta, but external AI criterion PASS
- [Beta Readiness Evidence Board](2026-05-10-beta-readiness-evidence-board.md) — living evidence tracker
- [External AI Run #003 Task Package](2026-05-10-external-ai-periodic-alpha-trial-run-003-task-package.md) — self-contained external AI task
- [Local Default Replacement Preflight](2026-05-10-local-default-replacement-preflight.md) — investigation-only; no default switch enabled; switch script implementation requires explicit user approval
- 操作手册：[Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)
- 端到端验证脚本：`scripts/alpha-trial-smoke.sh`（已修复可靠性）

**下一阶段 — Evidence Accumulation：**

1. **Run #005 + one more PASS run**：当前 3/5 beta-countable PASS，还差 2 次。
2. **日历跨度**：Run #001（05-09）→ Run #004（05-10）仍不足，需 ≥ 3 周后做正式 Beta 评估。
3. **Beta Readiness Evidence Board**：living document 已更新（[`evidence board`](2026-05-10-beta-readiness-evidence-board.md)），每次 trial 后继续更新。
4. **不扩产品面**：不扩 UI/Web/MCP/新语言。
5. **Local default replacement**：仅 preflight，不启用；下一步必须先获得用户批准，再实现 `scripts/local-default-switch.sh`。

**验收清单参考：**
- [Production Trial Acceptance Checklist](2026-05-09-production-trial-acceptance-checklist.md) — 逐项确认

**Rust CALLS 后续（低优先级，大部分在 stop-line 后）：**
- ~~`crate::` 多段路径分类修复~~ ✅ 完成（Slice 48）
- ~~import binding 多重同符号消歧~~ ✅ 完成（Slice 51）
- 关联函数 resolution：8 unresolved（含 stop-line 外部 crate type 方法、derive-generated 方法）
- 低置信度 reason/confidence 矩阵审计
- call form 文档与 confidence 矩阵对齐
- ~~Enum::Variant() 分类修复~~ ✅ 完成（Slice 53）

**Priority 3 续 — Rust graph contract**
- ~~第 5 个 contract fixture inline-module~~ ✅ 完成（Slice 49）
- ~~第 6 个 contract fixture self-path~~ ✅ 完成（Slice 52）
- ~~第 7 个 contract fixture enum-variant~~ ✅ 完成（Slice 54）
- ~~第 8 个 contract fixture workspace-member~~ ✅ 完成（Slice 55）

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
	46. **Phase 2g — 关联函数 type-filtered 消歧** ✅ 完成（2026-05-08）：
	   - `resolve_associated_function` 中按 `impl_details.impl_target == type_name` 过滤方法匹配
	   - 解决同模块多类型同名方法导致的误判歧义（如 `DataProcessor::build` vs `RequestHandler::build`）
	   - 新增 c15-associated-function-disambiguation fixture（compile-valid, 4 calls, 2 associated-function resolved）
	   - Improvement: +1 resolved call（CrossFileSymbolIndex::build），65.7% → 65.8%（2339/3557）
	   - 全部测试通过（call comparison 23/23 fixtures, graph_contract 24/24, cangjie_inspect 18/18）

47. **Stage 3 — 第4个 Rust contract fixture (module-hierarchy)** ✅ 完成（2026-05-08）：
   - 新增 `fixtures/rust/module-hierarchy/`：多模块工程，有 crate:: 路径、super:: 路径、import 绑定 三种 CALLS 模式
   - 新增 7 个 contract tests → 总计 30 tests on 4 fixtures（Before: 23 tests on 3 fixtures）
   - 追平 Cangjie graph contract 的 4-fixture 覆盖水平（Cangjie 24 tests on 4 fixtures）
   - module-hierarchy：13 nodes, 15 edges, 6 edge types, 2 source files, 3 CALLS（crate-path + super-path + import-resolved）
   - 全部测试通过（no-feature 30/30 project_model_graph_contract, with-feature 30/30, cangjie_inspect 18/18, graph_contract 24/24）
   - QUALITY.md 同步更新：contract fixture 表 3→4 fixtures，测试数 23→30

48. **Slice 48 — crate:: 多段路径 AssociatedFunction 误分类修复** ✅ 完成（2026-05-08）：
   - 修复 `classify_callee` 和 `classify_text_callee` 中 `crate::` 路径分类逻辑
   - 路径 `crate::module::Type::method()` 现在正确分类为 `AssociatedFunction`（之前误分类为 `QualifiedPath`）
   - `crate::module::function()` 保持 `QualifiedPath`（不退化）
   - 新增 fixture `c16-crate-associated-fn`（compile-valid，2 source files，3 calls）
   - Improvement: +1 resolved associated-function call
   - 全部测试通过（no-feature + feature，call comparison 24/24 fixtures，graph_contract 30/30）
   - Preflight: `docs/plans/2026-05-08-rust-crate-path-associated-fn-misclassification-preflight.md`
   - Closure Review: `docs/plans/2026-05-08-rust-crate-path-associated-fn-misclassification-closure-review.md`

49. **Slice 49 — 第5个 Rust graph contract fixture (inline-module)** ✅ 完成（2026-05-08）：
   - 新增 `fixtures/rust/inline-module/`：inline module 结构，含嵌套 inline module + crate::/self::/super:: 调用
   - 新增 7 个 contract tests → 总计 37 tests on 5 fixtures（Before: 30 tests on 4 fixtures）
   - Graph 产出：12 nodes, 18 edges, 6 edge types（含 HAS_PARENT），8 symbols（含 2 module symbols）
   - 验证 inline module symbol nodes + HAS_PARENT 边（之前 fixture 未覆盖）
   - 已知限制：self::/super:: 调用 unresolved（modulePath flat limitation，已记录）
   - 全部测试通过（no-feature + feature，graph_contract 37/37）
   - Preflight: `docs/plans/2026-05-08-rust-graph-contract-inline-module-preflight.md`
   - Closure Review: `docs/plans/2026-05-08-rust-graph-contract-inline-module-closure-review.md`

50. **Slice 50 — impl 块泛型目标类型解析修复** ✅ 完成（2026-05-08）：
   - 修复 `parse_impl_header()` 中 `generic_type`/`scoped_type_identifier` 节点跳过导致 impl_target 丢失
   - `impl<'a> SameFileIndex<'a>` → `_impl_SameFileIndex`（之前为 `_impl_Unknown`）
   - Improvement: +5 resolved associated-function calls, associated-function resolved 2→7
   - Resolution rate: 65.8% → 65.9%（2344→2352/3571）
   - 全部测试通过（no-feature + feature，call comparison 24/24，graph_contract 37/37）
   - Preflight: `docs/plans/2026-05-08-rust-impl-generic-target-parsing-preflight.md`
   - Closure Review: `docs/plans/2026-05-08-rust-impl-generic-target-parsing-closure-review.md`

51. **Slice 51 — import binding 多重同符号消歧** ✅ 完成（2026-05-08）：
   - 修复 `resolve_free_function()` 中多个 import binding 指向同一 symbol 时误判为歧义
   - 例：两个函数各自 `use` 同一符号，两个 binding 解析到相同 target → 应解析而非标记 ambiguous
   - Improvement: +2 resolved free-function calls（resolve_import_target ×2）
   - FreeFunction unresolved: 16 → 14
   - 全部测试通过（no-feature + feature，call comparison 7/7，graph_contract 37/37）
   - Preflight: `docs/plans/2026-05-08-rust-import-binding-same-symbol-disambiguation-preflight.md`
   - Closure Review: `docs/plans/2026-05-08-rust-import-binding-same-symbol-disambiguation-closure-review.md`

52. **Slice 52 — 第 6 个 graph contract fixture（self-path）** ✅ 完成（2026-05-08）：
   - 新增 fixture `fixtures/rust/self-path/`：self:: 路径调用、模块层次结构、struct + impl
   - 新增 7 个 contract tests：quality_gates、node_kind_set、edge_kind_set、known_symbols、known_defines_edges、known_calls_edges、calls_endpoint_integrity
   - 验证 self::top_level_fn() → same-module CALLS edge
   - 覆盖 HAS_PARENT（5 条）、DESIGNATION、模块嵌套结构
   - Graph contract: 37→44 tests（5→6 fixtures）
   - 全部测试通过（no-feature + feature + graph_contract 44/44）

53. **Slice 53 — enum variant 提取 + Type::Variant 分类修复** ✅ 完成（2026-05-08）：
   - model.rs：新增 `SymbolKind::EnumVariant`
   - item.rs：enum_item 分支提取 variant 子符号（`extract_enum_variant_symbol`），parentId 指向 enum
   - calls.rs：`classify_callee` + `classify_text_callee` 检测 `Enum::Variant` 模式（callee name 首字母大写 → FreeFunction）
   - 173 enum variant symbols 提取，318 enum-constructor calls 全量 resolved（100%）
   - 修复 `CangjieParseError::ParseFailed` 分类：AssociatedFunction → FreeFunction，1/2 resolved
   - Symbol comparison 3 fixture expected JSON 更新（item-top-level, item-top-level-regression, item-parse-error）
   - 全部测试通过（no-feature + graph_contract 44/44 + call_expected_compare 7/7 + symbol_expected_compare 4/4）
   - Preflight: `docs/plans/2026-05-08-rust-enum-variant-extraction-preflight.md`
   - Closure Review: `docs/plans/2026-05-08-rust-enum-variant-extraction-closure-review.md`

54. **Slice 54 — 第 7 个 graph contract fixture（enum-variant）** ✅ 完成（2026-05-08）：
   - 新增 fixture `fixtures/rust/enum-variant/`：简单 variant、元组 variant、结构体 variant、impl 方法内 variant 调用
   - 新增 7 个 contract tests：quality_gates、node_kind_set、edge_kind_set、known_symbols、known_defines_edges、known_calls_edges、calls_endpoint_integrity
   - 验证 7 个 enum variant Symbol 节点 + HAS_PARENT 边（variant → enum）
   - 验证 CALLS 边：make_keypress → KeyPress（元组 variant 调用）
   - Graph contract: 44→51 tests（6→7 fixtures）
   - 全部测试通过（no-feature + feature + graph_contract 51/51）

55. **Slice 55 — 第 8 个 graph contract fixture（workspace-member）** ✅ 完成（2026-05-08）：
   - 新增 fixture `fixtures/rust/workspace-member/`：workspace Cargo.toml + 2 成员 crate（lib-a, lib-b）
   - lib-b 依赖 lib-a（path dependency），测试跨 crate 调用
   - 新增 7 个 contract tests：quality_gates、node_kind_set、edge_kind_set、known_symbols、known_defines_edges、known_calls_edges、calls_endpoint_integrity
   - 验证 workspace 节点 + CONTAINS_WORKSPACE 边（repo → workspace）
   - 验证 CONTAINS_PACKAGE（workspace → 2 个成员）
   - 验证跨 crate CALLS：lib-b::welcome → lib-a::greet, lib-b::make_point → lib-a::new
   - 验证 stdlib 外部 symbol：String::from、push_str、push
   - Graph contract: 51→58 tests（7→8 fixtures）
   - 全部测试通过（no-feature + feature + graph_contract 58/58）

56. **Slice 56 — Rust Production Readiness 综合审计 + QUALITY.md stats 刷新** ✅ 完成（2026-05-09）：
   - 全面 read-only audit：重跑 self-smoke、Cangjie 回归、graph contract 全量
   - QUALITY.md 更新：stats 表（symbols 783→838、graph nodes/edges/CALLS 填入实际值）、resolved call distribution 表刷新（新增 crate-path-resolved/super-path-resolved）、unresolved breakdown 更新
   - docs/plans/README.md 同步：当前状态总结更新为最新 stats
   - 确认所有 unresolved free-function（15）均在 stop-line 后（局部闭包/cfg-gated/跨模块 variant）
   - 全部测试通过：no-feature ~200 pass、feature ~330 pass、Cangjie fixture smoke 4/4 PASS、Cangjie graph contract 24/24、Rust graph contract 58/58
   - 0 duplicate, 0 dangling, deterministic for both Rust and Cangjie
   - cargo fmt --check + git diff --check clean

57. **Local Trial Packaging** ✅ 完成（2026-05-09）：
    - 新增 `scripts/build.sh`：一键构建 release binary（含 Cangjie 特性），支持 --debug / --no-cangjie
    - 新增 `scripts/smoke.sh`：8 步快速 smoke 验证（fmt + test + Rust/Cangjie CLI + quality + self-smoke）
    - README.md 新增 Local Trial 章节
    - 产品化 closure review 更新：residual gaps 状态刷新（2 个已修复标注，1 个新增 gap 已处理）
    - 零新增依赖，不改 GitNexus-RC / Tool / live repo
    - Preflight: `docs/plans/2026-05-09-local-trial-packaging-preflight.md`

58. **Analyze --strict Flag** ✅ 完成（2026-05-09）：
    - `analyze` 命令新增 `--strict` flag（默认 false）
    - strict 模式：分析完成后检查所有 quality gates，任一失败 → stderr 输出失败详情 → exit 1
    - 与 Cangjie inspect/graph `--strict` 行为对齐
    - 兼容 `--format json` 和 `--format gitnexus-rc`
    - 新增 4 个 integration tests（2 Rust + 2 Cangjie feature-gated）
    - productization_commands tests: 15 → 19
    - 零新增依赖，不改 GitNexus-RC / Tool / live repo
    - Preflight: `docs/plans/2026-05-09-analyze-strict-flag-preflight.md`

59. **Docs Consolidation** ✅ 完成（2026-05-09）：
    - QUALITY.md §--strict：新增 unified `analyze --strict` 文档（Rust + Cangjie），区分 Cangjie inspect/graph 行为和 unified analyze 行为
    - bridge-preflight.md：状态从 Preflight 改为 Implemented；§五 改为实现状态表（3/4 步骤已落地）；新增 changelog entry
    - smoke.sh Step 4/6：改用 `analyze --strict`（exit code 检查 + JSON 验证）
    - 零新增依赖，不改 GitNexus-RC / Tool / live repo
    - Preflight: `docs/plans/2026-05-09-docs-consolidation-preflight.md`

60. **Cross-repo Consumer Dry-run** ✅ 完成（2026-05-09）：
    - P1: GitNexus-RC 消费侧只读审计（11 个核心文件，覆盖全消费链）
    - P2: Bridge Compatibility Report：`docs/architecture/gitnexus-rc-consumer-dry-run.md`（~416 行）
    - P3: 4 个 consumer shape 测试（2 Rust + 2 Cangjie）：symbol kind 具体性 + edge confidence/reason
    - P4: 2 个 bridge adapter 修复：
      - Symbol `kind` 字段填入具体类型（非通用 "symbol"），从 Rust `symbolKind` 属性提取
      - BridgeEdge 新增 `confidence: Option<f64>` + `reason: Option<String>` 顶层字段
    - bridge_roundtrip: 20 tests pass（10 Rust + 10 Cangjie），productization_commands: 19 tests pass
    - 不改 GitNexus-RC / Tool / live repo，零新增依赖
    - Closure Review: `docs/plans/2026-05-09-gitnexus-rc-consumer-dry-run-closure-review.md`

61. **Consumer Dry-run follow-up — edge kind compatibility tests** ✅ 完成（2026-05-09）：
    - 新增 `assert_edge_kind_compatibility()`：GitNexus-RC RelationshipType (24) → bridge edge kind 兼容性对照
    - 直接兼容表（15 values）+ adapter 映射表（17 bridge→RC type pairs），含 GitNexus-RC 源文件路径引用
    - 2 个新测试：`bridge_rust_edge_kind_compatibility` + `bridge_cangjie_edge_kind_compatibility`
    - bridge_roundtrip: 22 tests pass（11 Rust + 11 Cangjie），0 unknown edge kinds detected
    - README.md 新增 `--format gitnexus-rc` CLI 使用示例
    - Commit: `7be534b`

62. **MCP opencode Real Client Test** ✅ 完成（2026-05-11）：
    - 在 opencode 真实客户端中配置 CodeLattice MCP sidecar
    - 发现并修复 pipe-buffer deadlock（`run_subcommand_with_timeout` + `run_script_with_timeout`）：stdout/stderr 改为后台线程读取，避免 ~64KB OS pipe buffer 满导致子进程死锁
    - 发现并修复 path deny list 误匹配：`cangjie-GitNexus-Index` 被误判为 `cangjie` 子目录，改用 path-component-aware 匹配
    - Rust 场景测试：982 symbols, 60 files, 1923 nodes, 分析 ~1.5s
    - Cangjie 场景测试：903 nodes, 3252 edges, 887 symbols, 分析 ~8s
    - opencode 配置已保留，备份在 `opencode.json.bak-20260511-114701`
    - Report: `docs/plans/2026-05-11-opencode-mcp-real-client-test.md`

63. **Cangjie Live CodeLattice Production Runway** ✅ 完成（2026-05-11）：
    - 新增 `ALLOWED_DENIED_SUBPATHS` — live repo deny-list 豁免 `runtime/cjgui` 子路径
    - 新增 `scripts/cangjie-live-codelattice-smoke.sh` — 多模式 smoke（--dry-run/--analyze/--mcp/--tool-ingest/--full）
    - Live cangjie analyze: 3,046 nodes, 7,693 edges, 2,887 symbols, 157 files, all 6 quality gates pass
    - Tool registry 新建 `cangjie-live-codelattice`（17,194 nodes, 52,522 edges, 197 clusters, 300 flows）
    - MCP 7/7 pass（cache_prewarm, project_overview, graph_overview, symbol_search, symbol_context, production_assist, cache_status）
    - Tool context/detect-changes 验证通过
    - 命名规范：`cangjie-live-codelattice`（推荐）vs `cjgui-index`（fixture）vs `cjgui`（legacy deprecated）
    - Plan: `docs/plans/2026-05-11-cangjie-live-codelattice-production-runway.md`
    - Closure: `docs/plans/2026-05-11-cangjie-live-codelattice-production-runway-closure.md`

64. **Cangjie Production Alias & Registry Hygiene** ✅ 完成（2026-05-11）：
    - Registry audit: 7 repos total, 2 ambiguous `cjgui` entries identified, `cangjie-live-codelattice` confirmed as recommended
    - New `scripts/cangjie-production-alias-check.sh` — stable window check (--status/--smoke/--full)
    - Stable window rules: ≤10 dirty = green, 11-50 = yellow, >50 = red
    - Production Alias Switch Plan: Phase A (docs prohibit bare cjgui) → Phase B (AI defaults) → Phase C (WebUI) → Phase D (cleanup)
    - Plan: `docs/plans/2026-05-11-cangjie-production-alias-switch-plan.md`
    - Current window: RED (114 dirty) — Phase A active, Phase B blocked until green

65. **Cangjie Phase B Default Entry Switch** ✅ 完成（2026-05-11）：
    - GREEN window confirmed: live cangjie HEAD `7759612`, dirty=10
    - Full smoke pass: analyze 3,073 nodes / 7,745 edges / 2,912 symbols / 159 files, MCP 7/7, bridge 3MB
    - Tool registry refreshed: cangjie-live-codelattice 17,377 symbols / 53,092 edges / 194 files
    - Phase B activated: execution AI defaults to `cangjie-live-codelattice`, bare `cjgui` forbidden
    - New docs: phase B plan, agent command snippet
    - Plan: `docs/plans/2026-05-11-cangjie-phase-b-default-entry-switch.md`
    - Snippet: `docs/plans/2026-05-11-cangjie-live-agent-command-snippet.md`

66. **CodeLattice Runtime Isolation Pack** ✅ 完成（2026-05-11）：
    - 新增 `scripts/promote-to-local-tool.sh`：显式构建 Rust+Cangjie release binary，并安装到 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
    - 稳定运行目录：`CodeLattice-Tool/codelattice-mcp.sh` + `bin/codelattice-cli` + `manifest.json`
    - Codex / opencode 应指向稳定运行目录，不再直接指向开发 checkout wrapper
    - 目的：开发区源码修改、debug rebuild、wrapper 改动不会影响正在使用的 AI IDE；只有 explicit promote 才更新运行版
    - Plan: `docs/plans/2026-05-11-codelattice-runtime-isolation-pack.md`
