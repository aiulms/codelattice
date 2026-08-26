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
