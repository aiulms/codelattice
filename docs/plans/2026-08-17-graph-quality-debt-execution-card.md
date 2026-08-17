# 静态图谱质量债清理 · 执行卡（2026-08-17）

隶属：`2026-08-17-graph-quality-roadmap-pack.md` 任务 2。

## Preflight 调查结论（证据）

1. **TS TYPE_USE dangling（根因已定位）**：`crates/typescript/src/graph.rs:406-416`
   对每条 TypeUse 引用直接产出 `target: "ref:TypeUse:<name>"` 合成 ID，该 ID
   从未有对应节点。复现（本机，TS-feature binary）：`apps/desktop` 分析得
   1044 边中 526 条 dangling（全部 TYPE_USE，占 50.4%）。CALLS 边 0 dangling
   （131/131 完好）。ArkTS crate 复用同一 `build_ts_graph`，同病。
2. **质量门缺口**：TS/ArkTS 分析路径调用 `compute_arkts_quality_gates`
   （`crates/cli/src/lib.rs:4721`），只含 duplicate_nodes / dangling_source /
   deterministic 三门；缺 dangling_target / duplicate_edges /
   calls_endpoint_integrity，因此 526 条 dangling 边对质量门不可见。
3. **unknown-confidence 口径失真**：`crates/cli/src/mcp_server.rs:6244-6281`
   把所有缺 `properties.confidence` 的边计为 unknown，包括确定性结构边。
   而 Rust 分析器惯例（已验证）：结构边 DEFINES/OWNS_SOURCE/CONTAINS_PACKAGE/
   HAS_PARENT 从不带 confidence，只有解析型边 CALLS/ACCESSES/DESIGNATION 带。
   因此 48.7% 是分母口径问题，不是缺赋值。
4. **"TS 基线陈旧"**：dev binary 默认不带 `tree-sitter-typescript` feature；
   用 `cargo build --features tree-sitter-typescript` 重建后基线即新鲜。

## Write Set

- `crates/typescript/src/graph.rs` — TYPE_USE 目标解析：
  - 显式 import 绑定（`import_target_by_file_name`）→ 目标文件内同名
    类型类符号（Class/Interface/Enum/TypeAlias/Namespace/Component）→
    唯一候选才出边，confidence 0.9 / reason `imported-type`；
  - 同文件同名类型符号兜底，confidence 0.95 / reason `same-file-type`；
  - 解析失败不出边（no-edge policy），记 `typescript-type-use-unresolved`
    诊断（按 file+name 去重并计数）；
  - TYPE_USE 边按 (source, target) 去重，保留首次 line 与 useCount。
- `crates/typescript/tests/type_use_resolution.rs` — 新增集成测试：
  跨文件导入类型 / 同文件类型 / 未解析类型（如 DOM 全局类型）三场景。
- `crates/cli/src/lib.rs` — `compute_arkts_quality_gates` 补齐
  duplicate_edges / dangling_target / calls_endpoint_integrity 三门
  （语义对齐 `compute_rust_quality_gates`）。
- `crates/cli/tests/arkts_typescript.rs` — TS 侧断言新门存在。
- `crates/cli/src/mcp_server.rs` — edgeConfidence 指标新增解析边口径字段：
  `structuralEdgeCount` / `resolutionEdgeCount` /
  `unknownConfidenceResolutionEdgeCount` / `unknownConfidenceResolutionEdgeRate`
  （additive，不改既有字段语义）；风险引导阈值（>0.3）与文案改用解析边口径。
- 本执行卡 + `CHANGELOG.md`。

## Forbidden Set

- 不给结构边（DEFINES/OWNS_SOURCE/CONTAINS_PACKAGE/HAS_PARENT 等）批量补
  confidence —— 与 Rust 惯例冲突，且属为指标硬编码。
- 不为未解析 TypeUse 创建合成 ref 节点；不删除/重命名既有指标字段。
- 不做完整类型推断；TYPE_USE 解析只允许 import 绑定 + 同文件符号表两级
  名称匹配（对齐 method-dispatch 低置信启发式边界）。
- 不改 Rust 分析器、snapshot schema、webui、GitNexus-RC。

## Stop-lines

- `arkts_analyze_quality_gates_pass`（fixtures/arkts）在新门下必须真实通过；
  若 ArkTS 增强层引入新的 dangling，修复数据而不是放宽门。
- 未解析 TYPE_USE 从"边"变"诊断"会降低 edgeCount 指标 —— 这是预期行为，
  不得为保指标而保留 dangling 边。
- 全量 `cargo test`（含 `--features tree-sitter-typescript` 路径）必须通过。

## 验证矩阵

1. `cargo fmt --check` + `git diff --check`
2. `cargo test -p gitnexus-typescript --features tree-sitter-typescript`（新增测试）
3. `cargo test`（全量）
4. `scripts/codelattice-precommit-check.sh`
5. 复跑 `analyze --root apps/desktop --language typescript`：dangling=0、
   新门全绿、edgeCount 变化记录在案

## Closure（2026-08-17）

### 实施内容（含执行卡范围外的必要连带修复）

1. **TYPE_USE 目标解析**（`crates/typescript/src/graph.rs`）：两级名称匹配
   （import 绑定 → 同文件类型符号，仅 Class/Interface/Enum/TypeAlias/
   Namespace/Component），解析成功出边并带 confidence（0.9/0.95）与
   reason（imported-type/same-file-type），按 (file, symbol) 聚合并记
   useCount；解析失败不出边，按 (file, name) 去重记
   `typescript-type-use-unresolved` 诊断。
2. **门对齐**（`crates/cli/src/lib.rs` `compute_arkts_quality_gates`）：
   补齐 duplicate_edges / dangling_target / calls_endpoint_integrity。
   新门在真实数据上暴露两个既有缺陷，按"修数据不放宽门"处理：
   - **重复三元组**：CALLS 按 (caller, callee) 聚合（lines/callCount），
     IMPORTS 按 (file, target) 聚合（合并 names/lines）——与 Rust 分析器
     的边形态对齐（Rust 侧 0 重复）。
   - **无 resolver 兼容分支的 module: 伪目标**：ArkTS CLI 路径原先传
     `None` resolver，全部 import 走 `module:<specifier>` dangling 目标。
     修复：`TsModuleResolver` 增加 `.ets` 扩展探测（相对导入/tsconfig
     目标/index 文件），ArkTS 路径接入 resolver；@kit.* 系统包按
     External 诊断处理（不出边）。
3. **unknown-confidence 口径**（`crates/cli/src/mcp_server.rs`）：
   edgeConfidence 新增 structuralEdgeCount / resolutionEdgeCount /
   unknownConfidenceResolutionEdgeCount/Rate（additive）；风险引导阈值
   与文案改用解析边口径。结构边不补 confidence（与 Rust 惯例一致，
   不为指标硬编码）。
4. **diagnosticCount 硬编码清零**（连带）：`build_arkts_summary` 此前
   硬编码 `diagnostic_count: 0`，违反 stats 防守规则；改为从
   graph.diagnostics 真实计数（合并原 build_shell_summary），TS/ArkTS/
   JS/C/Cpp/Python/Shell 的 analyze+summary 路径统一。
5. 新增测试：`crates/typescript/tests/type_use_resolution.rs`（3 个）；
   `arkts_typescript.rs` TS 用例补齐 6 门存在性+通过断言。

### 关键证据（apps/desktop，TS-feature binary）

| 指标 | 修复前 | 修复后 |
|---|---|---|
| edges | 1044 | 654 |
| dangling edges | 526（全部 TYPE_USE） | **0** |
| duplicate triples | 32（不可见） | **0** |
| 解析边 confidence 覆盖 | 仅 CALLS | 354/354 = 100% |
| 质量门 | 3 门（缺 target/dup/calls） | 6 门全绿 |
| diagnosticCount | 0（硬编码，实际 174） | 174（真实） |

fixtures：arkts/portable-smoke、arkts/cross-file（IMPORTS 3 条全解析到
真实 .ets）、typescript/{portable-smoke, tsx-smoke, path-alias-monorepo,
graph-diagnostics} 六门全绿。

### 验证矩阵执行结果

1. `cargo fmt --check` / `git diff --check`：PASS
2. `cargo test -p gitnexus-typescript --features tree-sitter-typescript`：
   17 pass / 0 fail（含新增 3 个）
3. `cargo test`（默认全量）：62 个 test result 全 ok，0 fail
4. `cargo test -p gitnexus-rust-core-cli --features tree-sitter-arkts,tree-sitter-typescript`：
   454 pass / 1 fail —— 唯一失败 `mcp_consistency_review_old_client_stale`
   经 git stash 验证在 HEAD 上即失败（`find_symbols` 子串匹配把
   `testOldClient` 命中 `OldClient` 查询），系 typescript-feature 门
   长期未跑的存量缺陷，非本轮引入；已记录，修复另行开卡。
5. `scripts/codelattice-precommit-check.sh`：ALL PASS（fmt/diff/
   productization 339 项/mcp concurrency/detect-changes smoke 17/17）

### 风险与边界

- Native detect-changes staged 口径判 **critical**（7 文件、29 变更符号、
  3 unknown hunks）：本轮确为跨 TS 图构建/CLI/MCP 指标的横切变更，
  critical 是合理信号而非噪音。由上述全量测试矩阵 + 真实 fixture 验证
  覆盖，单提交可回滚。
- edgeCount 下降（1044→654）是 dangling/重复边消除的预期结果，不是
  解析能力回退；解析信息保留在聚合属性与诊断中。
- ArkTS `@kit.*` 系统包 import 不出边（External 诊断）；oh_modules 工作区
  包解析未实现，属后续能力。
- 未动 Rust 分析器 confidence 惯例；未改既有指标字段语义（只增字段）。
