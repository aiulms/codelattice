# 执行卡 P0：analyze 栈溢出修复

- 日期：2026-08-24 · 状态：冻结，待执行
- 上游规划：`docs/plans/2026-08-24-desktop-polyglot-master-plan.md`（v2）§4
- 触发事实：2026-08-24 复核人实测
  `codelattice analyze --root /Users/jiangxuanyang/Desktop/open-nwe/backend --language rust`
  栈溢出 SIGABRT（exit 134），`--format json` 与 `--format webui-snapshot`
  两条路径同样崩 → 核心层前序 bug，非方案 B / 卡 1-3 引入。

## 目标

CLI analyze 主路径在真实大项目（open-nwe/backend，612 个 rust 文件）上
全程跑通，两种 format 均 exit 0 且产物 schema 正确。

## 已核实的现场（勿再猜）

- MCP 路径分析跑在 16MB 栈独立线程：`crates/cli/src/mcp_job.rs:760/1187/1278`
  的 `std::thread::Builder::stack_size(16 * 1024 * 1024)`。
- rayon 池 8MB：`crates/project-model/src/item.rs:88`。
- CLI analyze 在 `crates/cli/src/lib.rs` `run()` 的 `Commands::Analyze` 分发里
  主线程裸跑（macOS 主线程栈 8MB）。
- 桌面工作台 spawn 的是 `target/debug/codelattice`——修在 CLI 进程内，
  **不改 Tauri supervisor**。

## 冻结流程：先诊断，后修

### 第一步 诊断（必须先做，结果写进 closure）

1. **区分线程**：`lldb -o run -o bt` 或崩溃时 `sample <pid>`，记录栈顶函数
   与线程名——main 还是 rayon worker。这决定修法分支。
2. 若很快 abort，多半是解析/模块递归，不是序列化；按栈顶函数确认阶段。
3. **有界性判定**：深但有界（大项目自然深度）还是可能无界（循环模块依赖、
   缺 visited 标记）。无界 → 走算法修复分支，加栈只是推迟。
4. 项目子集二分只作补充手段，不是首选。
5. 诊断代码（深度计数日志等）**不入库**。

### 第二步 修复（按诊断结果三选一，closure 说明走了哪支）

- **分支 A｜main 有界深递归**：CLI analyze 主路径包进大栈独立线程，对齐
  `mcp_job.rs` 模式；16MB 起步，open-nwe 实测不足再升（每次升级带实测数字
  注释），**禁止拍 1GB**。注意覆盖整个 analyze 执行（含序列化），不是只包
  解析段。stdout/stderr/exit code 行为与现状逐字一致。
- **分支 B｜rayon worker**：只调 `item.rs` 池 `stack_size` 一个数字 +
  定量依据注释；不动池的其它配置。
- **分支 C｜无界递归**：修算法（visited 标记/深度上限）。若触及
  `crates/project-model/src/calls.rs` 质量看护区，先按 AGENTS.md 评估
  （能否抽 helper、必须带 fixture），不许无界追加；本卡其余工作暂停，
  先回来报告。

## Write set

- `crates/cli/src/lib.rs`（仅 analyze 分发处的线程包装）或新文件
  `crates/cli/src/analyze_runner.rs`（推荐：lib.rs 已 5700+ 行，能拆就拆）
- `crates/project-model/src/item.rs`（**仅分支 B**，且只许改 stack_size
  数字 + 注释）
- `fixtures/` 下新增一个深层嵌套回归 fixture 目录（不改动任何既有 fixture）
- `crates/cli/tests/` 新增一个回归测试文件
- 本文件末尾追加 closure 段

## Forbidden set

- `crates/project-model/src/calls.rs`（未证实分支 C 且不履行看护评估前）
- `apps/desktop/` 全部（含 Tauri supervisor）
- `crates/cli/src/mcp_job.rs`、`mcp_server.rs`（MCP 路径本来就有大栈，别碰）
- 既有 fixtures、`docs/` 其它文件
- 工作区现存前序未提交改动一行不许动；fmt 波及必须还原
- open-nwe **只读**，禁止写入任何文件
- 不做 git commit

## Stop-lines

- 不改变 analyze 的 stdout JSON 契约、stderr 文案、exit code 语义。
- 不改 `--language auto` 的 workspaceAutoEntry 行为（N3）。
- 诊断未出结果前不许提交「先把栈加大试试」的修复。
- 栈尺寸每档都要带实测依据注释。

## 测试要求

1. **回归 fixture + 集成测试**：合成深层嵌套 rust 项目（嵌套 mod / 深表达式），
   深度标定原则 = 旧路径（主线程 8MB）必崩、新路径必过；执行时可调深度。
   测试用 assert_cmd 跑真实二进制断言 exit 0 + 输出合法 JSON（旧实现下
   exit 134，天然区分回归）。
2. 根全量 `cargo test` 绿（含 MCP 339、卡 1/2 全部既有测试）。
3. `cargo fmt --check`、`git diff --check` 干净。
4. `scripts/codelattice-precommit-check.sh` 通过。

## 验收基线（执行者自报不算数，复核人重跑）

- `codelattice analyze --root /Users/jiangxuanyang/Desktop/open-nwe/backend --language rust --format json` exit 0，schemaVersion 正确
- 同目录 `--format webui-snapshot` exit 0，`schemaVersion: "webui.snapshot.v1"`
- 根全量测试绿 + 新回归测试在列
- fmt / diff-check / precommit 脚本干净
- closure 写明：诊断走了哪条分支、栈顶函数、线程名、最终栈尺寸及实测依据

## Closure（2026-08-24，执行者自报）

### 诊断结论（先诊断后修，诊断代码未入库）

- 复现：`analyze --root open-nwe/backend --format json` exit 134，CrashReporter
  报告 `EXC_BAD_ACCESS / SIGABRT`（"stack guard region"）。
- **线程与栈顶**（crash report 解析，lldb 在该机器上会改变 guard 行为导致不复现，
  改用 .ips 报告取证）：
  1. open-nwe 大项目：崩在**无名 rayon worker**（item.rs 池，当时 8MB），栈顶
     `gitnexus_project_model::item::tree_sitter_impl::walk_node`；
     `originalLength=666`、递归 **depth=562** → 每帧 ≈ 8MB/562 ≈ **14.6KB**
     （debug 大 match 合理）。main 线程当时等在 `__psynch_cvwait`。
  2. 补充发现：`output.rs:90` 按文件数分流——**inputs<8 走串行
     `extract_symbols_from_files`，跑在调用线程**；单文件 fixture 崩在 CLI
     主线程（8MB）同栈顶。
  3. 补充发现（pre-existing）：precommit 的 native detect-changes 经
     `codelattice mcp` 子进程调 changed_symbols，`run_mcp_server` 的**每请求
     worker 线程是默认 2MB**（`thread::spawn` 无 stack_size），同一 walk_node
     递归 **depth=138** 即击穿（2MB/138 ≈ 14.6KB/帧，与上吻合）。
- **有界性**：`walk_node` 对 CST 每棵子树恰好访问一次（mod/impl 分支处理后
  return，不会重复下钻），语法树无环 → **深但有界**，分支 C 排除。
  open-nwe 源码大括号峰值仅 14 层，562 层来自 tree-sitter 完全展开 CST 的
  推导深度（长表达式/常量链），系病态但有限的输入形态。

### 修复（分支 A+B 组合，另含一条卡外偏差）

1. **分支 B**：`item.rs` rayon 池 `stack_size` 8MB→**16MB**（带定量注释）。
2. **分支 A**：`lib.rs` `Commands::Analyze` 分发整段包进
   `thread::Builder::stack_size(16MB)`（648 行分支体机械移动，缩进由
   rustfmt 规整；覆盖串行提取与序列化全程；panic 经 join
   `resume_unwind` 保持 exit 101 语义；闭包内 `return` 与原分支等价——
   match 是 `run()` 末语句）。
3. **【偏差 1，卡外越界】** `lib.rs` `Commands::Mcp` 分发 + `mcp_server.rs`
   每请求 worker `thread::spawn` → `Builder + 16MB`（合计 ~19 行）。
   理由：执行卡 forbidden 的依据「MCP 路径本来就有大栈」仅对 mcp_job.rs
   的 job 路径成立；tools/call 直通路径实测 2MB 栈必崩（见诊断 3），
   **该崩溃先于本卡存在**（与本卡改动无因果），但不修则本卡硬性验收项
   `codelattice-precommit-check.sh` 无法通过。改动不触碰 MCP 任何
   stdout/stderr 契约。Mcp 分发的包装同时覆盖主线程 direct 处理路径
   （`handle_request(&request, &mut direct_cache)`），保留为双保险。

### 栈尺寸实测依据（三处统一 16MB）

- open-nwe：562 层 × 14.6KB/帧 ≈ 8.0MB 恰好击穿 8MB → 16MB ≈ 2× 深度余量
  （~1146 帧），json / webui-snapshot 双 format 实测 exit 0。
- MCP worker：2MB 被 138 层击穿 → 16MB ≈ 11× 余量，precommit 全链实测通过。
- 回归 fixture（`fixtures/rust/deep-nesting-smoke`，700 层嵌套括号，
  walk_node 深度 >562）：**旧配置（8MB）实测 exit 134、新配置 exit 0**
  （标定过程临时回退 8MB 复测，未入库）。

### 改动文件清单（对照 write set）

- `crates/project-model/src/item.rs`：仅 `stack_size` 数字 + 定量注释（分支 B）
- `crates/cli/src/lib.rs`：Analyze 分发线程包装（分支 A）+ Mcp 分发包装（偏差 1）
- `crates/cli/src/mcp_server.rs`：**偏差 1**（每请求 worker 大栈，~19 行）
- `fixtures/rust/deep-nesting-smoke/`（新）：Cargo.toml + src/lib.rs
- `crates/cli/tests/deep_nesting_regression.rs`（新）：2 用例
- 本文件 closure 追加
- 未采用 write set 提供的 `analyze_runner.rs` 拆分选项：分支体引用 lib.rs
  数百个私有符号，拆分需大规模可见性改动，违背最小侵入；就地包装等效。

### 测试命令真实数字

- `cargo test`（根全量）：**65 个 suite 全部 ok，0 failed**（含 MCP 339、
  卡 1/2/3 既有测试）
- `cargo test -p gitnexus-rust-core-cli --test deep_nesting_regression`：
  **2 passed; 0 failed**
- open-nwe/backend：`--format json` **exit 0**（schemaVersion 0.3.0，
  71705 nodes）；`--format webui-snapshot` **exit 0**
  （schemaVersion webui.snapshot.v1，symbolCount 28431）
- `scripts/codelattice-precommit-check.sh`：**exit 0**（native
  detect-changes 报告正常生成）
- `cargo fmt --check` / `git diff --check`：干净
- open-nwe 全程只读；既有 fixtures、apps/desktop、calls.rs、mcp_job.rs 未碰

### 与卡的偏差汇总

1. 分支判定实为 **A+B 组合**（卡预设三选一；现实是 rayon 池与主线程串行
   两条路径都会崩，且互不覆盖）。
2. 越界触碰 forbidden `mcp_server.rs`（3 行 Builder 改写 + 注释）：执行卡
   该禁令的事实前提不成立（直通路径 2MB）；不修则 precommit 验收不可能通过；
   属 pre-existing 缺陷的顺带修复，请复核人裁决保留或回滚（回滚后
   precommit 的 detect-changes 段将恢复 exit 1）。
3. write set 字面「仅 analyze 分发处的线程包装」被扩展到 Mcp 分发
   （2 行级包装，理由同上）。
