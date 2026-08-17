# 图谱质量与仓库治理路线 pack（2026-08-17）

## 背景

Workbench P0 于 2026-08-05/06 完成四轮返工后全部 gate 关闭（最终提交
`6868e9b5`），master 与 gitcode/master 同步，跟踪文件无未提交改动。本 pack
规划 P0 收尾后的下一阶段工作，按以下顺序推进：

1. 未跟踪文件清理（治理噪音消除）
2. 静态图谱质量债清理（TS 基线 / dangling edge / unknown-confidence）
3. `calls.rs` text fallback 第三刀拆分
4. Workbench P1 图谱导航 preflight（本轮只产出计划，不实施）

## 动机与证据

- P0 closure 记录：native detect-changes 宽口径屡次判 `critical`，原因之一是
  工作区存在 20+ 个与本任务无关的未跟踪文件（`.cursor/`、`.omo/`、`.planning/`、
  `.workbuddy/`、讨论稿、`webui/mockups/`），`.gitignore` 未覆盖工具目录。
- P0 closure 与视觉返工卡反复记录的静态风险：TypeScript 分析基线陈旧、4 条既有
  dangling edge、48.7% unknown-confidence edge（非调用类边 78.4%）。这些不是
  本轮符号影响升级造成，但持续把后续变更审查顶到 HIGH/CRITICAL。
- AGENTS.md quality watch：`crates/project-model/src/calls.rs` 当前 1984 行，
  text fallback（约 376 行）暂留待第三刀；继续新增 CALLS 策略前必须先拆。
- `webui/mockups/project-workbench-v3.html` 被 P0 计划第 5 行引用为原型参考，
  属于应入库的治理历史。

## 任务 1：未跟踪文件清理

Write Set：
- `.gitignore`（新增 `.cursor/`、`.omo/`、`.planning/`、`.workbuddy/`）
- `docs/讨论-AI变更回执与用户理解.txt`、`docs/讨论-外部复核意见-v2.txt`（原位入库）
- `webui/mockups/`（HTML 原型 + PNG，原位入库）

Forbidden Set：
- 不移动/重命名讨论稿与 mockups 路径（P0 计划已按当前路径引用）。
- 不修改任何 Rust/TS 代码、snapshot、schema。
- 不提交 `.cursor/` 等工具目录内容本身，只忽略。

验证：`cargo fmt --check` + `git diff --check` + native detect-changes
（docs-only 提交，无代码语义变更）；提交后 push gitcode master。

## 任务 2：静态图谱质量债清理

范围（具体执行卡在 preflight 调查后冻结）：
- TypeScript 分析基线陈旧的根因与刷新机制。
- TS 静态图中 4 条 dangling edge 的来源与修复（对齐 graph schema v0.2
  “CALLS edge must not be dangling” stop-line）。
- unknown-confidence 边占比过高的分层归因：哪些 edgeKind 缺 confidence 语义、
  哪些是分析器未赋值；优先修复“分析器可确定却未赋值”的部分。

Stop-lines（继承 AGENTS.md）：
- 不为降指标而批量硬编码 confidence 值；必须有语义依据。
- dangling edge 修复不得以丢弃边为默认手段，除非符合 no-edge policy。
- 不做 type inference / trait solving / macro expansion。

## 任务 3：calls.rs text fallback 第三刀

- 目标：将 `crates/project-model/src/calls.rs` 中约 376 行 text fallback
  提取为独立模块，行为等价，全量测试通过。
- 已知约束：text fallback 与 resolve 逻辑共享 `resolve_free_function` /
  `resolve_associated_function`，需评估共享函数的可见性提升（`pub(crate)` 或
  移至共享模块），不允许为拆分而复制两份逻辑。
- 验证：`cargo fmt --check`、`cargo test` 全量、CodeLattice self-analysis
  CALLS resolution rate 不回退（基线 65.7%，2338/3557）。

## 任务 4：Workbench P1 图谱导航（仅 preflight）

- 方向：社区折叠、分层布局、语义聚类（P0 closure “剩余边界” 挂账项）。
- 本轮只产出 preflight 文档，冻结执行卡另起。

## 总 Stop-lines

继承 AGENTS.md MVP stop-lines 全部条款；本 pack 不涉及 live repo、
GitNexus-RC runtime、生产环境。每任务完成后按序提交并 push gitcode master，
push 失败记录错误继续低风险工作，不做 destructive git 操作。
