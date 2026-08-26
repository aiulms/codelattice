# 执行卡：桌面端项目挑选器（多语言卡 3）

- 日期：2026-08-24（v2，吸收外部复核意见后修订）
- 状态：冻结，待执行
- 依赖：卡 1（快照节点 language 字段，已交付）、卡 2（`codelattice inspect`，已交付复核通过）、方案 B（`--format webui-snapshot`，已交付）
- 前置文档：`docs/plans/2026-08-24-polyglot-language-identity-preflight.md`、`docs/plans/2026-08-24-workspace-inspect-command-execution-card.md`

## 目标

打通桌面工作台「选文件夹 → 体检 → 挑项目 → 出图」全链路：用户选一个文件夹后，先跑
`codelattice inspect` 体检，按体检结果决定直接分析还是弹出项目挑选器，消灭
App.tsx 两处写死的 `"rust"`（497 行 analyze 调用、543 行 snapshotMeta）。

一次只分析一个项目。不合并多语言快照（后续独立卡）。

## 链路现状（已核实，勿再猜）

- 前端：`apps/desktop/src/App.tsx` `selectAndAnalyze`（约 487 行起）→
  `transport.selectProjectDirectory()` → `transport.analyze(root, "rust")` →
  2s 轮询 `analyzeStatus()` → Completed 后 `loadSnapshot(publishedSnapshotId)`。
- transport 接口：`apps/desktop/src/types.ts` `DesktopTransport`（249 行起）；
  三份实现：`transport/desktop-transport.ts`（Tauri invoke）、`fake-transport.ts`
  （vitest 用）、`http-transport.ts`（analyze 本就是 stub，抛
  `"not available on legacy runner"`）。
- Tauri 侧：`commands/analyzer.rs` `workbench_analyze` → `analyzer.rs`
  `AnalyzerSupervisor`（spawn `codelattice analyze --format webui-snapshot`，
  发布前守卫只认 `webui.snapshot.v1`）；命令注册在 `main.rs` 约 115 行。
- 体检命令（卡 2 已交付）：`codelattice inspect --root <dir> --format json`，
  信封 `codelattice.workspaceInspection.v1`，三桶
  `projects` / `sourceOnlyAreas` / `unsupportedAreas`，行字段
  `relativePath` / `language?` / `confidence` / `evidence` / `sourceFileCount` /
  `analyzable` / `reason?` / `recognition?`。遍历有 MAX_WALK_DEPTH=5 /
  MAX_ENTRIES=5000 上限，毫秒级，可同步调用。
- panels 命名惯例为 kebab-case：`structure-tree.tsx` / `chat.tsx` / `graph-pane.tsx`。

## 冻结契约

### 1. Tauri 新增 `workbench_inspect` 命令

- 文件：`apps/desktop/src-tauri/src/commands/analyzer.rs`（追加，不动已有函数），
  注册进 `main.rs` invoke_handler。
- 行为：同步 `Command::new(codelattice_bin).args(["inspect","--root",root,"--format","json"]).output()`；
  二进制路径与 `workbench_analyze` 同口径（`common::repo_root().join("target/debug/codelattice")`）。
  **不套** analyze 那条 `nice -n 10`（毫秒级扫描，没必要）。
- 校验：exit 非零 → Err(stderr 摘要)；stdout JSON 的 `schemaVersion` 必须逐字等于
  `codelattice.workspaceInspection.v1`，否则 Err 并点名实际值（对齐 analyze 的
  发布前守卫风格）。校验通过后**原样透传** envelope JSON 给前端，不在 Rust 侧
  重新建模字段——契约单一事实源在 core CLI。
- 不进 AnalyzerSupervisor、不占 analyze 单任务 gate、不可取消。
- analyzable 已由 CLI inspect 算好，桌面只透传；**禁止**在 Tauri/前端再做
  feature 判定。

### 2. 前端 transport 与类型

- `types.ts`：新增 `WorkspaceInspection` / `InspectionRow` 类型，字段与信封逐字对应
  （可选字段用 `?`，与「缺席即未知」语义一致）；`DesktopTransport` 接口加
  `inspect(root: string): Promise<WorkspaceInspection>`。
- `desktop-transport.ts`：`invoke("workbench_inspect", { root })`。
- `fake-transport.ts`：**默认体检信封逐字如下**（不冻这份，现有 126 个 vitest
  里点「分析项目」的用例会对不上——0 行会让失败横幅用例变成「未发现可分析项目」，
  ≥2 行会停在挑选器进不了 analyze）：

```json
{
  "schemaVersion": "codelattice.workspaceInspection.v1",
  "projects": [{
    "name": "project",
    "relativePath": ".",
    "language": "rust",
    "confidence": "certain",
    "evidence": { "kind": "manifest", "file": "Cargo.toml" },
    "sourceFileCount": 1,
    "analyzable": true
  }],
  "sourceOnlyAreas": [],
  "unsupportedAreas": []
}
```

  这样默认路径 `analyze("/fake/project", "rust")` 与现状完全一致。多项目/零项目
  场景的测试用 `override async inspect()`，与现有 `override async analyzeStatus()`
  同一模式。
- `http-transport.ts`：与现有 analyze stub 完全一致，
  `throw new Error("not available on legacy runner")`，不另造文案。

### 3. App.tsx 流程改造（唯一允许大改的前端文件）

`selectAndAnalyze` 改为两段：

1. 选目录 → `transport.inspect(root)`。
2. 收集候选行 = `projects ∪ sourceOnlyAreas` 中 `analyzable === true` 的行：
   - **恰好 1 行** → 不弹挑选器，直接分析（保持现有单项目体验零回归）。
   - **0 行** → 走现有 `analyzeFailureText` 错误横幅，文案说明「未发现可分析项目」；
     若存在 unsupported 行，附一句「检测到 N 个暂不支持的语言区域」。
   - **≥2 行** → 进入挑选态，渲染挑选器，等用户选择或取消。
3. 分析目标与标签（消灭两处写死后的新口径）：
   - `analyzeRoot`：`row.relativePath === "."` 用对话框根目录原值；否则按根路径里
     已有的分隔符（`/` 或 `\`）拼接一个小 helper。**禁止** `import path from
     "node:path"`（Vite/webview 没有 node 模块）。
   - `language` = 该行 `language`。analyzable 行在 CLI 契约里必有 language；
     **若缺席视为 inspect 契约破坏，错误横幅报错，禁止回退 `"rust"`**。
   - `rootLabel` = `row.name ?? relativePath 最后一段`，**不要**用对话框根目录名
     （用户选仓库根再点 backend 时，标题仍显示 `codelattice · rust` 就是错的）。
4. 挑选器取消 → 回 idle，不报错。inspect 本身失败 → 错误横幅（含后端错误文本）。
5. 用户选完一行 → 挑选器卸载，再进现有 analyze 轮询（轮询逻辑一行不改）。

### 4. 挑选器 UI

- 新文件 `apps/desktop/src/panels/project-picker.tsx`（kebab-case，对齐
  `structure-tree.tsx`；**不要** `ProjectPicker.tsx`），内联面板，不做模态不做路由。
- testid 冻结：面板 `project-picker`；取消按钮 `picker-cancel`；可点行
  `picker-row-${relativePath}-${language}`；unsupported 折叠区 `picker-unsupported`。
- 主列表 = `projects ∪ sourceOnlyAreas` 全部行：`analyzable: true` 可点，显示
  `name ?? relativePath` + `language` + `sourceFileCount` + 证据摘要（manifest
  文件名或扩展名直方图）；`analyzable: false` 的行（如没编对应 feature 的
  python 区）**主列表内灰显**不可点，附 `reason`。
- `unsupportedAreas` **默认折叠**（真实仓库 unrecognized 行极多，不折叠会刷屏），
  展开后只读。
- 样式走现有 `styles.css` 追加 class，不引入新依赖、不改 theme.ts。

## Write set（允许新增/修改的文件）

- `apps/desktop/src-tauri/src/commands/analyzer.rs`（追加 `workbench_inspect`
  及其 `#[cfg(test)]` 测试）
- `apps/desktop/src-tauri/src/main.rs`（仅 invoke_handler 注册一行）
- `apps/desktop/src/types.ts`
- `apps/desktop/src/transport/desktop-transport.ts`
- `apps/desktop/src/transport/fake-transport.ts`
- `apps/desktop/src/transport/http-transport.ts`
- `apps/desktop/src/App.tsx`
- `apps/desktop/src/panels/project-picker.tsx`（新）
- `apps/desktop/src/styles.css`（追加）
- `apps/desktop/src/e2e/workbench-production.test.tsx`（按需适配 + 新增用例）
- `apps/desktop/src/e2e/` 下可新增一个 picker 专项测试文件

## Forbidden set（不许碰）

- `crates/` 全部（CLI inspect 契约已冻结，本卡纯桌面侧）
- `apps/desktop/src-tauri/src/analyzer.rs` **整个文件**——`workbench_inspect` 的
  测试写在 `commands/analyzer.rs` 的 `#[cfg(test)]`，**自备假 CLI 脚本**，不为
  复用去抽 `analyzer.rs` 测试模块里的 `fake_cli`
- `apps/desktop/src-tauri/src/snapshots.rs` / `query_store.rs` / `models.rs`
- `fixtures/` 全部
- 工作区现存的前序未提交改动（moduleGraph 等几十个文件），一行不许动；
  fmt 波及必须还原
- 不做 git commit

## Stop-lines

- 不合并多语言快照；一次 analyze 仍是一个 (root, language)。
- 不给 analyze 加 `auto` 语言入口；挑选器就是 auto 的替代品。
- 不在前端/Tauri 自建语言检测、扩展名表或 feature 判定；一切语言信息来自
  inspect 信封。
- 不改 `webui.snapshot.v1` 契约。
- unsupported/unrecognized 行永不可选。
- **CLI** `target/debug/codelattice` 缺 tree-sitter feature 时行会
  `analyzable:false`（工作台 spawn 的是这份二进制，不是 workbench 自己），
  挑选器如实灰显，不做「帮你重编」之类的动作。

## 测试要求

1. **src-tauri**（`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`）：
   现有 18 个保持绿；新增 `workbench_inspect` 测试（假 CLI 脚本自备，与
   analyzer.rs 测试的 fake_cli 同模式但不抽共享）：正常透传 v1 信封、拒绝错误
   schemaVersion、exit 非零报 Err。
2. **vitest**（`cd apps/desktop && npx vitest run`）：现有 **126 个原用例全绿**
   （新增用例另计）；新增覆盖：
   - 单 analyzable 行直通不弹挑选器，analyze 收到的 language 是该行语言；
   - 多 analyzable 行弹挑选器，点选后 analyze 收到正确 root+language，
     snapshotMeta 的 language 与 rootLabel 随之更新（rootLabel 来自所选行，
     不是对话框根目录）；
   - 0 analyzable 行出错误横幅；
   - 挑选器取消回 idle；
   - unsupportedAreas 默认折叠。
3. `cd apps/desktop && npx tsc --noEmit` 干净。
4. 两个 fmt：`cargo fmt --check`（根）与
   `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check`。
5. `git diff --check` 干净。

## 验收基线（执行者自报不算数，复核人重跑）

- src-tauri：18 + 新增全绿
- vitest：126 原用例 + 新增全绿
- tsc / fmt×2 / diff-check 干净
- 手工 smoke：`apps/desktop` dev 起应用：
  - 选 `fixtures/mixed/inspect-smoke` → **必须出挑选器**（默认 CLI 二进制下
    `backend` rust 与 `scripts/tools` shell 两行可点，python 灰显，java/.xyzfoo
    在折叠的 unsupported 区）；不是直通，不要含糊。
  - 选仓库根 → 多项目挑选器。
  - 确认 analyze 后图正常出、头部标签 = 所选项目的语言与名字。

## Closure（2026-08-24，执行者自报）

### 实际改动文件清单（与 write set 一一对应）

- `apps/desktop/src-tauri/src/commands/analyzer.rs`：追加 `workbench_inspect`
  命令 + 私有 `inspect_with_cli`（同步 spawn、schemaVersion 逐字守卫、原样透传）
  + `#[cfg(test)] mod inspect_tests`（假 CLI 脚本自备，未抽共享、未碰
  `src/analyzer.rs`）
- `apps/desktop/src-tauri/src/main.rs`：invoke_handler 追加一行注册
- `apps/desktop/src/types.ts`：`InspectionEvidence` / `InspectionRow` /
  `WorkspaceInspection` + `DesktopTransport.inspect(root)`
- `apps/desktop/src/transport/desktop-transport.ts`：
  `invoke("workbench_inspect", { root })`
- `apps/desktop/src/transport/fake-transport.ts`：默认体检信封逐字照抄执行卡
- `apps/desktop/src/transport/http-transport.ts`：analyze 同款 stub 文案
- `apps/desktop/src/App.tsx`：`selectAndAnalyze` 拆两段（inspect → 直通/报错/
  挑选态）+ `startAnalyze(root, row)`（语言、rootLabel、失败文案全部来自所选行；
  两处写死 `"rust"` 消灭）+ `joinUnderRoot` helper（无 node:path）+ 挑选器渲染
- `apps/desktop/src/panels/project-picker.tsx`（新）：主列表可点/灰显 +
  unsupported 折叠只读；testid 按冻结（`project-picker` / `picker-cancel` /
  `picker-row-${relativePath}-${language}` / `picker-unsupported`）
- `apps/desktop/src/styles.css`：追加 picker class（含 dark 主题），未改既有规则
- `apps/desktop/src/e2e/project-picker.test.tsx`（新）：5 用例（单行直通语言正确、
  多行挑选 root+language+snapshotMeta、0 行横幅、取消回 idle、unsupported 折叠）

### 测试命令真实通过数字

- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`：
  **21 passed; 0 failed**（18 原有 + 3 新增 inspect 命令测试）
- `cd apps/desktop && npx vitest run`：**131 passed (131)**，18 test files
  （126 原有用例全绿 + 5 新增）
- `cd apps/desktop && npx tsc --noEmit`：无输出（干净）
- `cargo fmt --check`（根）：干净
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check`：干净
- `git diff --check`：干净

### 与卡的偏差

1. **失败文案的 root 口径**：`startAnalyze` 内 `analyzeFailureText` 使用
   `analyzeRoot`（按所选行拼接后的路径）而非对话框根目录——单候选直通且
   relativePath 为 "." 时两者相同；多候选时更准确。轮询/加载/会话逻辑未改。
2. **rootLabel 对 "." 行的兜底**：卡规定 `row.name ?? relativePath 最后一段`；
   relativePath 为 "." 时无最后一段可用，兜底为对话框根目录名（此时行本就
   代表根，不违反「选仓库根点 backend 时显示根名」的禁止项本意）。
3. 手工 smoke 的应用已由执行者在后台启动（`npm run tauri dev`），数据面
   （inspect-smoke 四区行、python 灰显、unsupported 折叠）已由卡 2 CLI smoke
   与本卡 e2e 双重覆盖；UI 点选留待复核人按验收基线重跑。
