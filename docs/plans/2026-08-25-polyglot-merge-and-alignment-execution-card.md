# 执行卡：多语言合并出图 + 收尾对齐（总体规划 P2+P3+P4 合并卡）

- 日期：2026-08-25 · 状态：冻结，待执行
- 上游：`docs/plans/2026-08-24-desktop-polyglot-master-plan.md`（v2，第三方复核已并入）
- 前置已交付：方案 B（webui-snapshot 转换器）、卡 1（节点 language）、卡 2（inspect）、
  卡 3（桌面挑选器）、P0（栈溢出修复）、P1（全量语言构建）
- 用户指令：P2/P3/P4 不再分卡，一次交付。执行仍按本卡三段顺序推进，
  每段自验后再进下一段。

## 总目标

选 open-nwe 根目录 → 一次操作 → 一张覆盖全部可分析语言的合并图；
Python snapshot-gen 脚本退役；MCP 与桌面同源。

## 第一段：P2 合并多语言快照

### 已核实的现场（勿再猜）

- 转换器 `crates/cli/src/webui_snapshot.rs`：`MAX_NODES=150` / `MAX_EDGES=300`
  （16-17 行）；`relationKey = sha256(source\0kind\0target)`（217-226 行）；
  文件节点 id 形如 `file:src/lib.rs`；信封顶层 `language` 是单数（118/127 行）。
- 体检函数 `gitnexus_workspace_model::inspect_workspace_inventory` 可直接复用；
  CLI 信封组装在 `crates/cli/src/workspace_inspect.rs`（`language_analyzable`
  也在这，可复用做 analyzable 过滤）。
- P0 已把 analyze 主路径包进 16MB 大栈线程（lib.rs 约 4533 行）；
  Mcp 分发与每请求 worker 同档。
- 桌面链路：卡 3 挑选器 → `transport.analyze(root, language)` →
  Tauri `workbench_analyze` → supervisor spawn `analyze --format webui-snapshot`
  → 发布守卫认 `webui.snapshot.v1`。
- 前端语言徽标已存在（structure-tree.tsx:118-133、g6-adapter.ts:112），
  本卡不含渲染工作。

### 冻结契约

**1. 新动词 `analyze-workspace`**（独立子命令；**禁止**沾 `auto` 名——
`--language auto` 是多项目根返回 workspaceAutoEntry.v1 的既有语义，N3 不动）。

```
codelattice analyze-workspace --root <dir> [--format webui-snapshot]
```

行为：
1. 内部调 `inspect_workspace_inventory` 拿三桶；
2. 候选 = `projects ∪ sourceOnlyAreas` 中 analyzable 行（用
   `workspace_inspect::language_analyzable` 同一函数判定，不另写）；
3. 逐行 analyze（root+relativePath、该行 language），拿到**未截断**的内部图；
4. 内存合并 → 一次转换 → 一次截断 → 输出一个 webui.snapshot.v1 信封。
5. 进度写 stderr（如 `分析中 typescript (2/3): frontend`）；stdout 只出最终 JSON。
6. **每段语言之间检查取消条件**（对 CLI 即响应外部 kill 的速度；编排循环不得
   有一段跑完才退出）。
7. **失败策略**：单语言失败 → 跳过并继续，在 `limitations` 里逐条结构化点名
   （语言 + 路径 + 原因）；全部失败才 exit 非 0。

**2. 合并规则**：
- 在**未截断的 0.3.0 子图**上合并（先截断再并 = 一堆残片，open-nwe 规模必假绿）。
- 节点 id 加项目命名空间；`file` 路径改为**仓库相对**（`backend/src/lib.rs`），
  结构树靠它并目录。
- **relationKey 必须按最终 id 重算**，禁止沿用子图旧 key；契约测试更新已知答案。
- 边：只做子图并集 + 共享目录树祖先；**不发明跨语言边**（N1）；禁止 dangling。

**3. 信封加法（v1 不变）**：
- 可选顶层 `languages: string[]`（字母序去重）；合并快照写 `languages[]`，
  **不写**单数 `language`；单语言 analyze 快照维持原样只写 `language`。
- 可选顶层 `inspectionSummary`：从 inspect 信封裁剪的三桶摘要（让前端能展示
  「还有 N 块未纳入」），不是自由字符串。
- `generatedFrom` 安全段 + `limitations` 声明「跨语言调用未解析」。

**4. 桌面接线**（最小面）：
- 挑选器加「全部分析（合并）」入口；单项目直通/挑选行为保持不变。
- transport 加 `analyzeWorkspace(root)`；Tauri `workbench_analyze` 加一个
  可选布尔参数（或等价加法）让 supervisor spawn `analyze-workspace`；
  发布守卫无需改（产物仍是 webui.snapshot.v1）。
- 合并完成后 snapshotMeta：language 显示 `languages.join(" · ")`，
  rootLabel 用对话框根目录名。
- pin/query store 键空间不变（合并后仍是一条 job-* 快照）。

### P2 验收

- open-nwe 根：`analyze-workspace` exit 0，信封含 `languages`（至少 rust +
  typescript + python + shell）、`inspectionSummary`；节点 id 全部带项目前缀；
  relationKey 重算后有契约测试已知答案；无 dangling（写断言测试）。
- 单语言失败场景 fixture：部分合并成功 + limitations 点名。
- 桌面：open-nwe 根 → 挑选器 → 全部分析 → 出图，结构树按仓库相对路径并目录。
- 根全量测试绿；65+ 新增 suite 全绿。

## 第二段：P3 Python snapshot-gen 脚本退役

- `scripts/codelattice-snapshot-gen.py` 与 core 转换器**逐字段 diff**
  （redact / explore / quality 等段）；core 缺的补齐到 `webui_snapshot.rs`。
- `fixtures/webui-snapshots/*.json` 改用 CLI 重新生成；
  `webui/contract-tests/` 必须继续绿。
- 删脚本 + 改文档引用；确认无 CI/外部调用方（用 grep 证明）。
- **若 diff 出 core 无法等价的段**：停下来报告，不许悄悄降级退役。

## 第三段：P4 MCP 对齐

- MCP 侧复用 `webui_snapshot.rs` 转换器（消灭 MCP/桌面输出漂移）；**加法**，
  不改 MCP 现有输出 schema（N2）。
- `codelattice_workspace` 加 `mode=inspect`（或等价加法），agent 可拿到
  workspaceInspection.v1。
- MCP 339 个测试必须继续全绿。

## Write set

- `crates/cli/src/`：新模块（如 `workspace_analyze.rs`）+ lib.rs 分发接线 +
  `webui_snapshot.rs`（languages/inspectionSummary/合并入口/缺段补齐）
- `crates/cli/tests/` 新测试文件；`fixtures/` 新增合并/失败场景 fixture
  （不动既有）
- `apps/desktop/`：App.tsx、transport 三份、types.ts、project-picker.tsx、
  styles.css、Tauri `commands/analyzer.rs` + `analyzer.rs`（仅 spawn 参数加法）、
  e2e 测试
- `crates/cli/src/mcp_server.rs` / `mcp_job.rs`：仅第三段加法
- `scripts/codelattice-snapshot-gen.py`（退役时删除）+
  `fixtures/webui-snapshots/`（CLI 重生成）+ 文档引用
- 本文件末尾 closure

## Forbidden set / Stop-lines

- `crates/project-model/src/calls.rs`；`detect_by_extensions` 等扫描器语义；
  `--language auto` 与 workspaceAutoEntry 行为；MCP 现有输出 schema。
- 不发明跨语言边；不产 dangling 边；stats 实算。
- open-nwe 只读；不做 git commit；前序未提交改动不许误碰，fmt 波及必须还原。
- 三段顺序执行，每段自验（该段测试 + fmt）后再进下一段；某段卡住就停，
  把已完成段的证据留下。

## 验收基线（复核人重跑）

每段的验收命令真实输出 + open-nwe 端到端（analyze-workspace CLI 一次、
桌面点选一次）+ 根全量测试 + src-tauri + vitest + tsc + fmt×2 + diff-check。

## Closure —— 第一段 P2（2026-09-13，执行者自报；P3 / P4 未开工）

### 实际改动文件清单（与 write set 一一对应）

- `crates/cli/src/workspace_analyze.rs`：合并编排核心 `build_merged_snapshot`
  （analyze 闭包可注入，锁定失败跳过策略）+ CLI 薄壳；节点 id 项目命名空间
  （根 `root::`）、file 改仓库相对、绝对路径归一化（属性缺席时以项目根为锚，
  压掉命名空间 `::` 前缀让转换器从 id 抠出 `/Users/...` 的回退）；后处理写
  `languages[]`（字母序去重）、删单数 `language`（顶层 + summary）、补
  `inspectionSummary` 三桶摘要 + `mergedProjectCount` + `failedProjects`、
  limitations 声明跨语言未解析 + 跳过逐条点名；`#[cfg(test)]` 4 个单测
  （部分失败点名 / 全部失败 Err / 根命名空间 / 绝对路径归一化）
- `crates/cli/src/lib.rs`：`Commands::AnalyzeWorkspace` + 16MB 大栈线程分发
  （前轮已落，本轮未改）；`webui_snapshot.rs` 仅 `pub(crate)` 两个 helper
- `crates/cli/tests/analyze_workspace.rs`：4 个端到端契约测试
- `apps/desktop/src/types.ts`：`analyzeWorkspace(root)`、`SnapshotData.languages /
  inspectionSummary`、analyzeStatus 增 `progress`/`mode`
- `apps/desktop/src/transport/{desktop,fake,http}-transport.ts`：三份同步加法
  （http 仍 stub 抛错）；desktop 传 `{ root, language: "", merge: true }`
- `apps/desktop/src-tauri/src/analyzer.rs`：`start()` 加 `merge` 参数（merge 走
  `analyze-workspace --root <dir> --format webui-snapshot`，不传 `--language`）；
  stderr 从 `Stdio::null()` 改 piped + 读线程存最后一行（`running_progress()`，
  不改 supervisor 锁模型）；`RunningJob` 加 `is_merge`/`progress`；job_id 加
  进程内序列号（修并发 spawn 时间戳撞车互相 truncate 产物的偶发竞争）；stale
  的「界面挑选流程尚未支持」文案更新；新增 3 测（merge spawn 参数无
  `--language` / 单项目参数保持 / 合并 v1 信封照常发布）
- `apps/desktop/src-tauri/src/commands/analyzer.rs`：`workbench_analyze` 加可选
  `merge`；复核轮改为 `language: Option<String>`（单项目空 language 显式报错）；
  `analyze_status` 加 `mode`/`progress`
- `apps/desktop/src/App.tsx`：轮询抽成共享 `pollAnalyzeCompletion` + 新增
  `startAnalyzeWorkspace`（meta：rootLabel=对话框根名、language=languages
  join " · "）；复核轮修复：立即首拍、tick 互斥、卸载清 interval、
  starting/Running 均可取消
- `apps/desktop/src/panels/project-picker.tsx` + `styles.css`：「全部分析（合并）」
  按钮（testid `picker-analyze-all`）+ head-actions 样式
- `apps/desktop/src/e2e/project-picker.test.tsx`：新增合并用例（点全部分析 →
  只调 `analyzeWorkspace(对话框根)` 且不调 `analyze`、标题 = 根名 + languages
  join）；原 5 用例零回归

### 测试命令真实通过数字

- `cargo test -p gitnexus-rust-core-cli --test analyze_workspace`：**4 passed**
- `cargo test -p gitnexus-rust-core-cli --lib workspace_analyze`：**4 passed**
- `cd apps/desktop && npx vitest run src/e2e/project-picker.test.tsx`：**6 passed**
- `cd apps/desktop && npx vitest run`（上轮全量）：**132 passed**，18 files
- `cd apps/desktop && npx tsc --noEmit`：干净
- `cd apps/desktop/src-tauri && cargo test`：**24 passed**（连跑 4 次稳定）
- `git diff --check`：干净；`workspace_analyze.rs` / src-tauri fmt --check 干净
  （仓库级 `cargo fmt --check` 有既有 fmt 债，非本卡文件未代跑）
- 根 CLI crate 全量仅 `project_model_call_expected_compare` c17 红——HEAD
  worktree 复跑同样红，**原先就红**（stdlib trait 提取区，禁碰），非本卡引入

### CLI 端到端（open-nwe，只读）

- 全 feature 二进制（`scripts/codelattice-build-workbench-cli.sh`，47MB vs 默认
  28MB；inspect 四语言 analyzable）
- `analyze-workspace --root ~/Desktop/open-nwe --format webui-snapshot`：
  **exit 0，约 9s**；`languages=["python","rust","shell","typescript"]`；
  `mergedProjectCount=12`；合并规模 61708 节点 / 56487 边，预览 150 节点 / 58 边
  （先合并后一次截断）；dangling=0；机器路径文件 0；stderr 进度
  `分析中 python (7/12): backend/src/core/debug_adapter` 等 12 行

### 桌面点选证据（2026-09-13，tauri dev 实点，非自报推断）

1. 点「分析项目」→ 原生面板选 open-nwe 根 → **挑选器弹出**：12 个可分析行
   （backend rust 428 文件、frontend typescript 403 文件、4 个 python 区、
   4 个 shell 区…）+「▸ 暂不支持的区域 · 323」折叠，与 CLI inspect 一致
2. 点「全部分析（合并）」→ 挑选器卸载，**状态条出现「分析中 rust (1/12):
   backend」**（CLI stderr 进度行透传到 UI），「取消分析」按钮在 Running 可见
3. 约 18s 后 Completed：**标题栏 =「open-nwe · python · rust · shell ·
   typescript」**（rootLabel=对话框根名，语言字母序 join）；无 IPC /
   missing field / language is required 报错
4. **结构树按仓库相对路径并目录**：backend → src → api → chat_handlers →
   *.rs（rust 徽标），同树含 frontend 入口 init ThemeManager.ts（typescript
   徽标）；**无 Users/jiangxuanyang**；图是一张（Package/Module 6 · File 50 ·
   Symbol 90 单一分层视图），不是按语言各一张
5. 磁盘产物 `target/workbench-snapshots/job-…-0.json`：webui.snapshot.v1、
   `languages` 四语言、无单数 `language`、root=open-nwe、dangling=0
6. **卡 3 零回归抽查**：同一挑选器点 backend rust 行 → 标题变「backend · rust」
   （行名 + 行语言），结构树为 backend 项目内相对路径（src/api/...），仪表盘
   144 节点 · 55 边（≠合并 150·58）；磁盘新增 `job-…-1.json` 带
   `language: "rust"`、root=open-nwe/backend —— 走的是单项目 analyze，未误走 merge
7. 未逐项点验：取消按钮在 starting 瞬间的可见性（单项目/合并都跑太快；
   代码条件 `Running || starting` + vitest 覆盖）、GUI 内真实点「取消分析」

### 与卡的偏差

1. 失败场景测试用注入式 analyze 闭包单测（`build_merged_snapshot`）而非纯
   真实数据 fixture：默认 feature 下 rust 分析无确定性失败路径（manifest 错误
   降级为 diagnostic），shell 区入候选必有 ≥2 文件（必成功）——真实数据造不出
   稳定单语言失败；跳过/点名/部分合并/全部失败 exit≠0 由注入测试直接锁定
2. 「边 = 子图并集 + 共享目录树祖先」实现为：子图并集 + 仓库相对路径经
   moduleGraph 目录聚合呈现共享树；未新造目录祖先图边（避免发明边）
3. 复核轮三处修复（language Option、transport 传空 language、App 轮询四点）
   由复核轮落地，本轮核对在位并纳入 closure
4. `inspectionSummary` 前端已随快照加载（类型在），但未做「还有 N 块未纳入」
   的专门 UI 展示——卡 P2 验收清单未强制渲染，留待后续卡

### 已知小问题

1. 根级项目的 repo 容器 file 显示 `"."`（压 id 抠路径绝对回退的产物），语义
   正确、展示略生硬（卫生级）
2. App 轮询器已修为卸载清 interval；tick 互斥防重叠开-session（复核轮已修）
3. 全仓 fmt 债 + c17 pre-existing red 可能误导复核人（归因见上）
## Closure —— 第二段 P3（2026-09-13，执行者自报；P4 未开工）

### 硬门结论：通过（无 core 无法等价的段）

同一份 analyze JSON（fixtures/rust/portable-smoke，绝对根）下 Python 脚本 vs
CLI `--format webui-snapshot --redact-root` 逐字段 diff：剩余 23 处全部可解释，
无残缺段——

- 卡 1 / P2 冻结超集：图节点 `language`、moduleGraph `languages[]`、顶层
  `language`（单语言快照契约）
- stats 实算修正（Python 旧缺陷，按 AGENTS.md stats 规则修正而非照搬）：
  `quality.passedGateCount/failedGateCount`（Python 读 `status` 字段恒 0/0，
  core 按 `passed` 实算 7/0）；`quality.diagnosticsSummary`（Python 读不存在的
  analyze 顶层 diagnostics 恒 null，core 从 graph.diagnostics 实算）；
  **`cleanup.deadCode/unreachableCandidateCount`（Python 真 bug：CALLS 边的
  target 误写入 `target_ids` 而非 `call_targets`，候选数恒等于符号总数；
  core 按段语义实算未被调用符号，portable-smoke 9→5）**——以 Rust 单测
  `cleanup_counts_uncalled_symbols_from_call_edges` 作为回归锁
- 格式差异：`toolVersion`（"codelattice 0.17.0-beta.2" → "0.17.0-beta.2"）、
  `generatedFrom.generationMethod`（"cli-aggregate-phase-a" →
  "cli-format-webui-snapshot"）
- limitations notes 超集（core 多 .h 误伤说明一条）

quality 输入无需外部 JSON：`analyze.qualityGates` 与 `codelattice quality`
命令的 7 道门逐字段一致（已实测），转换器进程内直接取用。

### 实际改动文件清单

- `crates/cli/src/webui_snapshot.rs`：信封升级为 Phase A enriched 全量段——
  新增 `build_quality_section`（qualityGates + graph.diagnostics 实算）、
  `build_explore_section`（500 符号/200 文件上限、kindLabel、exported、
  topFiles）、`build_cleanup_section`（CALLS 语义实算）、
  `build_release_review_section`、`workflow_presets_section`（10 预设原表）、
  insights.reviewFirst（热点前 3 文件）；summary 增 `moduleCount` 并对齐
  Python normalized_node_kind 计数口径；`--redact-root` 全局路径脱敏
  （redact_path/_looks_like_absolute_path/redact_all_paths 移植 + 顽固片段
  替换 + relationKey 按脱敏后端点重算）+ moduleGraph 后置于脱敏；新增 8 个
  单测（含 Python cleanup bug 回归锁、module-id 规则冻结用例，承接退役的
  scripts/test_module_graph.py 覆盖）
- `crates/cli/src/lib.rs`：`analyze` 加 `--redact-root` 开关（仅
  webui-snapshot format 有效，其他 format 显式报错）
- `scripts/webui-snapshot.sh`：改为 CLI 瘦包装
  （`analyze --format webui-snapshot --profile full [--redact-root]`）；
  `--full/--include-*` 保留接受（恒全开），`--compact/--no-enrichment` 移除
  （无仓内调用方，误用时显式报错）
- `scripts/webui-snapshot-smoke.sh`：去掉 gen.py/python3 前置检查
- `scripts/codelattice-snapshot-gen.py`、`scripts/test_module_graph.py`：删除
  （纯函数覆盖移入 Rust 单测）
- `webui/contract-tests/tests/snapshot-contract.test.mjs`：模块图期望值 oracle
  从 exec Python 脚本改为同规则 JS 内联移植（不变量本身不变：count 之和 ==
  可聚合边数），头部注释更新
- `fixtures/webui-snapshots/*.json`（7 个，非 6 个——目录实有 7 份）：全部用
  CLI 新路径重生成，零手改数字
- `docs/webui/README.md`、`docs/webui/webui-snapshot-contract.md`：聚合链路
  叙述改到 CLI

### 测试命令真实通过数字

- `cargo test -p gitnexus-rust-core-cli --lib webui_snapshot`：**22 passed**
- `cargo test -p gitnexus-rust-core-cli --test analyze_workspace`：**4 passed**
- `cargo test -p gitnexus-rust-core-cli --lib workspace_analyze`：**4 passed**
- `cd apps/desktop && npx vitest run src/e2e/project-picker.test.tsx`：**6 passed**
- `cd webui/contract-tests && npm run test:node`：**22 passed; 0 failed**
- `bash scripts/webui-snapshot-smoke.sh`：**8 pass 0 fail**（7 语言 + arkts-auto，
  无 /Users 泄漏、quality/workflowPresets/explore 齐全）
- `bash scripts/webui-snapshot.sh --root fixtures/rust/portable-smoke
  --language rust --output - --redact-root`：exit 0，15 顶层键
- `git diff --check`：干净；`webui_snapshot.rs` fmt 干净

### 与卡的偏差

1. 未按"6 个 portable-smoke"重生成，实为目录里的全部 7 份（rust/ts/js/c/cpp/
   python/shell）
2. 三处 stats 修正（cleanup 计数、passedGateCount、diagnosticsSummary）不是
   字节级照搬 Python——Python 侧是缺陷（详见硬门结论），照搬违反 AGENTS.md
   stats 实算规则；契约测试不冻结这些数字
3. `webui-snapshot.sh` 的 `--compact/--no-enrichment` 未保留（CLI 恒输出全量
   段无法表达"关闭"）；仓内无调用方，误用显式报错
4. 契约测试的模块图 oracle 从"exec Python 脚本"改为"同规则 JS 内联"——测试
   强度不变（同一不变量、同一归并规则，规则由 Rust 单测冻结），否则删 .py 后
   测试自身跑不了

### 已知小问题

1. `quality.gates` 沿用 analyze.qualityGates 原样透传（gateName/passed/detail
   键名与 v1 fixture 一致），未做键名归一
2. explore 的 symbol `line/endLine` 沿用 Python 行为恒输出（缺省 null），未按
   "缺席即未知"省略——保持信封形状一致优先

## Closure —— 第三段 P4（2026-09-13，执行者自报；三段全部完成）

### 转换器探查结论（先探后改）

grep `mcp_server.rs` / `mcp_job.rs`：**无自建 relationKey / moduleGraph / 150-300
截断 / explore / cleanup / workflowPresets，无平行 snapshot 组装**——MCP 从未
自拼 webui 快照，无需删重复实现。出现的 `deadCodeCandidateCount`
（mcp_server.rs:27307）属于 codelattice_delete_code review 域自己的
`summary.candidateCount` 转写，与 snapshot cleanup 段不同源、无漂移——
**查过，无转抄**。因此按卡面预案加显式 opt-in 出口而非删除。

### 实际改动文件清单

- `crates/cli/src/mcp_server.rs`：
  - `codelattice_workspace` 加 `mode=inspect`：复用
    `workspace_inspect::build_inspection`（与 CLI inspect 同一份
    `codelattice.workspaceInspection.v1` 信封，analyzable/reason 由 CLI/model
    计算，MCP 零 feature 判定；未触碰未提交的
    `build_inspection_excluding`/`--exclude` WIP）；毫秒级同步，不套 job；
    `compact_workspace_inspection` 助手实现 compact 契约——保留
    schemaVersion/三桶计数/行的 path+language+analyzable+reason，unsupported
    裁剪到前 20 行但必须留 totalCount + truncated 标记 + 说明（非 compact
    原样透传；facade 惯例缺省 compact=true）
  - `codelattice_project` 加 `mode=snapshot`：显式 opt-in 出
    `webui.snapshot.v1`——复用 `cache.get_or_analyze` 的进程内 analyze 结果 +
    `webui_snapshot::convert_analyze_result`（P3 后的唯一事实源），零新拼图；
    多语言合并不在 MCP 实现（走 CLI analyze-workspace / 桌面）；默认
    graph/overview/impact/full/job 路径与 schema 不变（N2）
  - tools/list：两个工具的 mode enum 同步加 `inspect` / `snapshot`
  - 引导文案加法：workspace 根的 recommendedNextAction 与 rootDiagnosis
    cautions 增加「mode=inspect 先盘点可分析项目」推荐；现有 graph 默认路径
    不变
- `crates/cli/tests/mcp_server.rs`：3 个新契约测试（inspect v1 信封 +
  analyzable 透传 / compact 裁剪留 count+说明且非 compact 原样 / snapshot 出
  webui.snapshot.v1 全量段且 auto 语言走既有 resolve 链）
- `docs/mcp/ai-usage-guide.md`：workspace 表加 inspect、project 表加
  snapshot、Monorepo 段补 inspect 用法
- `crates/cli/src/mcp_job.rs`：**零改动**（inspect 同步毫秒级，无需 job 化）

### 测试命令真实通过数字

- `cargo test -p gitnexus-rust-core-cli --test mcp_server`：**342 passed; 0
  failed**（此前基线 339 + 新增 3，与卡面 339 对账一致）
- P2/P3 回归：analyze_workspace 4/4、workspace_analyze lib 4/4、
  webui_snapshot lib 22/22、project-picker vitest 6/6、src-tauri（上轮 24/24
  未受本轮影响）、`git diff --check` 干净、mcp_server.rs fmt 干净
- 真实 stdio 实测（非测试桩）：对仓库根本身调
  `codelattice_workspace mode=inspect compact=true` → facade.v1 + result=
  workspaceInspection.v1，三桶 140/24/134，unsupported 裁剪 20 行 +
  truncated=true，root 字段保留

### 与卡的偏差

1. mode=inspect 的信封放在 facade 信封的 `result` 字段内（facade.v1 +
   result=workspaceInspection.v1）——与全部 facade mode 的输出惯例一致；
   v1 信封本身原样透传、可按 `result.schemaVersion` 识别。若调用方要裸信封，
   CLI `codelattice inspect` 保持可用
2. inspect 缺省 compact=true（facade 全家惯例 `facade_compact_default`），
   全量信封需显式 `compact:false`——测试锁定了两种形态
3. mode=snapshot 落在 `codelattice_project`（单项目）而非 workspace——多项目
   合并按卡面走 CLI analyze-workspace，MCP 不另编合并

### 已知小问题

1. compact 裁剪行保留 name 字段（存在时），未严格只留四字段——name 对 agent
   选项目有用，属超集
2. mode=snapshot 每次调用走 cache（analyze 一次的成本），无进度反馈——大项目
   建议仍走 job 类模式，snapshot 定位是「点名单份快照」
