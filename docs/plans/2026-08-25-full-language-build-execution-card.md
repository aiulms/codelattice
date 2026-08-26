# 执行卡 P1：全量语言构建（工作台 CLI）

- 日期：2026-08-25 · 状态：冻结，待执行
- 上游规划：`docs/plans/2026-08-24-desktop-polyglot-master-plan.md`（v2）§5
- 触发事实：open-nwe 点验时 frontend（typescript，506 文件）在挑选器灰显
  `language-support-disabled-in-this-binary`——CLI `default =
  ["tree-sitter-extraction"]`（crates/cli/Cargo.toml），dev 二进制只有 rust+shell。
- 前置：P0 栈溢出修复已交付复核通过（2026-08-24），「出图」验收可勾选。

## 目标

桌面工作台 spawn 的 `target/debug/codelattice` 具备全部语言 feature，
`codelattice inspect` 各语言行 `analyzable: true`（feature 维度）。

## 已核实的现场（勿再猜）

- feature 定义在 `crates/cli/Cargo.toml`：
  `tree-sitter-extraction`（默认，rust）/ `tree-sitter-typescript` /
  `tree-sitter-javascript`（含 typescript）/ `tree-sitter-python` /
  `tree-sitter-c` / `tree-sitter-cpp` / `tree-sitter-cangjie` /
  `tree-sitter-arkts`；shell（gitnexus-shell）非 optional，恒在。
- analyzable 判定在 `crates/cli/src/workspace_inspect.rs` 的
  `language_analyzable`，按 `cfg!(feature=...)` 编译期自适应——**feature
  编进去就自动变 true，不需要改任何源码**。
- 桌面端二进制路径口径：`common::repo_root().join("target/debug/codelattice")`，
  `cargo build -p gitnexus-rust-core-cli --features ...` 产物正好落在这个
  路径，桌面零改动。

## 冻结契约

### 1. 构建脚本

新文件 `scripts/codelattice-build-workbench-cli.sh`：

```bash
cargo build -p gitnexus-rust-core-cli --features "\
tree-sitter-extraction,\
tree-sitter-typescript,\
tree-sitter-javascript,\
tree-sitter-python,\
tree-sitter-c,\
tree-sitter-cpp,\
tree-sitter-cangjie,\
tree-sitter-arkts"
```

- **显式列表冻结，禁止 `--all-features`**（workspace 以后新增 optional feature
  会被默默编进来）。
- debug profile（对齐桌面 dev 现状）；脚本失败要非零退出。
- 脚本头部注释写明：用途（桌面工作台全语言二进制）、为何显式列表、
  与 `codelattice-precommit-check.sh` 的关系（precommit 用默认 feature，
  本脚本不影响它）。

### 2. 文档

`docs/guides/` 下新增一篇短文档（或并入既有桌面 dev 文档，执行时看哪篇
合适，closure 写明选择）：工作台 CLI 构建方法、feature 清单、「构建不过」
清单（见 §3）。

### 3. vendor 风险处置

`tree-sitter-cangjie` / `tree-sitter-arkts` 依赖 vendor 语法源码。若某个
feature 在本机编不过：

- 从脚本列表移除该 feature，在文档「构建不过」清单逐条记录（feature 名、
  编译错误摘要、日期）；
- **禁止改 vendor 源码、禁止改对应 crate 的 build.rs 去绕过**；
- inspect 会把该语言如实报 `language-support-disabled-in-this-binary`，
  这就是设计好的降级行为。

### 4. 不造探测机制

不新增二进制 feature 探测/版本自检。inspect 的 analyzable 已经天然反映
feature 状态，再造一套是双份漂移。

## Write set

- `scripts/codelattice-build-workbench-cli.sh`（新）
- `docs/guides/` 一篇文档（新或并入既有）
- 本文件末尾追加 closure 段

## Forbidden set

- `crates/` 全部（包括 `Cargo.toml`——feature 定义已存在，不需要改）
- `apps/desktop/` 全部
- vendor 语法源码、各语言 crate 的 build.rs
- 既有 fixtures、scripts/ 其它脚本
- 工作区现存前序未提交改动一行不许动
- open-nwe 只读；不做 git commit

## 测试与验收

1. 运行构建脚本：exit 0（或按 §3 记录构建不过项后 exit 0）。
2. `./target/debug/codelattice inspect --root /Users/jiangxuanyang/Desktop/open-nwe`
   → typescript / python 行 `analyzable: true`（cangjie/arkts 视 §3 结果）。
3. **出图验收**（P0 已通路，可勾选）：`analyze --root open-nwe/frontend
   --language typescript --format webui-snapshot` exit 0 且
   `schemaVersion: "webui.snapshot.v1"`。若失败：报错必须是分析层错误
   （带诊断信息），不允许是栈溢出或缺 feature——后者说明本卡没做到位。
4. 根 `cargo test`（默认 feature）全绿——本卡不改代码，绿是回归证明。
5. `git diff --check` 干净；`git status --short` 证明没碰 forbidden set。

## 验收基线（执行者自报不算数，复核人重跑）

- 构建脚本实跑 exit 0
- open-nwe inspect 的各语言 analyzable 状态
- frontend typescript analyze exit 0 + schema
- 根全量测试绿
- closure 写明：feature 实编清单、构建不过清单（若有）、文档落在哪篇

## Closure（2026-08-25，执行者自报）

### feature 实编清单

八个 feature 一次全部编译通过（macOS arm64 / darwin 25.6.0，
增量构建 11.5s）：`tree-sitter-extraction`（rust）、`tree-sitter-typescript`、
`tree-sitter-javascript`、`tree-sitter-python`、`tree-sitter-c`、
`tree-sitter-cpp`、`tree-sitter-cangjie`、`tree-sitter-arkts`；shell 非
optional 恒在。**vendor 风险未发生**：cangjie / arkts 在本机编译通过。

### 构建不过清单

无（空表，见文档占位段；换平台若编不过按表登记）。

### 文档落点

新增 `docs/guides/workbench-cli-build.md`（选择理由：guides/ 下现有四篇
均与桌面 dev 无关——README / ai-mcp-tool-guide / ai-prompt-cookbook /
workflow-presets，无既有桌面 dev 文档可并入）。

### 验收命令真实结果

- `scripts/codelattice-build-workbench-cli.sh`：**exit 0**
  （`Finished dev profile in 11.47s`）
- `inspect --root open-nwe`（实跑产物）：**python / rust / shell / typescript
  全部 `analyzable: true`，无 reason**
- `analyze --root open-nwe/frontend --language typescript --format
  webui-snapshot`：**exit 0**，`schemaVersion: "webui.snapshot.v1"`，
  language=typescript，305 sourceFile / 2145 symbol / 5 moduleGraph 模块
- 根 `cargo test`（默认 feature）：**65 suite 全 ok，0 failed**（回归证明）
- `cargo fmt --check` / `git diff --check`：干净
- `git status --short`：本任务仅新增
  `scripts/codelattice-build-workbench-cli.sh`、
  `docs/guides/workbench-cli-build.md`、本 closure；crates/（含
  Cargo.toml——其 M 状态系前序会话 sha2 依赖行，非本任务）、apps/desktop/、
  vendor、既有 fixtures 零触碰；open-nwe 全程只读。

### 与卡的偏差

无。脚本形状、feature 列表、文档位置均按卡执行；未新增探测机制。
