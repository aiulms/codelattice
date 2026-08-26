# 执行卡 P0：bin→lib 调用边缺失修复 + 外部评审毛刺清偿

- 日期：2026-08-26 · 状态：已修复并 promote（beta.2）
- 复核收尾（同日，外部复核 pass 后的增量）：① docs-only 口径统一 ——
  `risk.overallRisk` 与 `crossProjectRisk` 此前不受第零层封顶约束，同一
  输出三字段打架（summary=low / overallRisk=HIGH / crossProject=critical），
  已用 `is_docs_only_diff` 公共判定统一压齐；② 根因表述修正 —— 单独的
  `HEAD~5...` 本机 git 可接受，报 usage 的是旧实现组合出的
  `git diff HEAD HEAD~5...`；③ 补齐 base-ref 与 workspace auto 两个修复
  的自动化回归（`crates/cli/tests/detect_changes_regressions.rs`，
  CLI 端到端走 git 子进程）；④ 清理测试 unused 警告。
- 触发事实一（调用边）：外部评审（Codex）与 dsh agent 复测双源确认，
  `apps/stock-core`（bin+lib 同包结构）CALLS 边 30 条，`main.rs::run →
  instance.rs::discover_existing` 这类包内跨文件调用零边；GitNexus 对同一
  符号给出正确 incoming calls。impact/detect-changes 对该结构系统性低估
  爆炸半径。
- 触发事实二（毛刺清单）：外部评审 6 条，5 条属实（版本纪律 / vendored
  诊断噪音 / CLI-MCP 面不对称 / 质量门跨语言不齐 / docs-only 高风险），
  1 条非本仓缺陷（dsh-mcp-client 重连）。
- 追加触发（执行中确认）：评审转述的 `detect-changes --base-ref` bug
  属实且为三层叠加 —— ① `{base}...` 三点拼接非法 git 命令；② `--merge-base`
  与 diffMode=head 的 `HEAD` 组合成恒空 diff；③ auto 语言探测在多项目
  workspace 根误选散脚本 Python 并 fail-fast（not compiled）。

## 根因（已定位并全部修复）

调用边缺失为四层叠加，全部实测验证：

1. `crates/project-model/src/source.rs` — bin+lib 双 target 包内非
   target-root 文件返回 `AmbiguousTarget { package: None }` → 符号进不了
   crate-wide 调用索引。**Fix A**：有 lib target 时默认归 lib
   （confidence 0.80），纯多 bin 仍不猜。
2. `crates/project-model/src/imports.rs:332` — `use <own-pkg>::x` 首段
   小写判为 External 直接 skip。**Fix B**：对照 Cargo.toml 包名
   （TargetModel.name，非目录名——第一版从目录名反推在 /tmp 路径下失配）
   命中时改走 lib crate root 解析；mod-chain 失败（末段是符号非模块）时
   symbol 级 crate-wide 唯一名兜底（`own_package_import` 标记 +
   `UseReexportResolved`，confidence 0.80）；`pub use <module>::x` 的
   re-export 同样按 crate:: 重写。
3. `crates/project-model/src/calls.rs` 遍历白名单 — 内层调用藏在
   `field_expression` / `reference_expression` / `try_expression` 等包裹
   节点里（`&fn(..).map_err(..)?` 形态），白名单外整棵子树丢弃。
   **Fix C**：白名单加入 field/reference/parenthesized/await/closure/try
   六种包裹节点。
4. **Fix C 引发的栈回归**：遍历加深后 rayon 全局池默认 2MB 栈被
   open-nwe/backend 击穿（crash 复现）。calls 提取改用与 item.rs 同口径
   的 16MB 局部池，crash 消除。

## 修复效果（实测）

| 项目 | 修复前 | 修复后 |
|---|---|---|
| stock-core CALLS | 30 | 9,357（run→discover_existing 边恢复） |
| open-nwe/backend CALLS | ~4,475 | 62,452 |
| `--base-ref HEAD~5` | git usage 报错 | 19 files / 85 symbols 正确产出 |
| open-nwe 根 auto 探测 | Python not compiled 崩 | Ambiguous 引导显式指定 |
| 股票插件整仓 sourceFiles | 5,114（含 vendored） | 230（.tools 排除） |
| 股票插件整仓诊断 | 7,773 | 2,809 |
| docs-only diff riskLevel | high | low 封顶 |
| 版本 | 0.17.0-beta.1（TS 大修未 bump） | 0.17.0-beta.2 |

质量门全部保持通过（stock-core 7/7、open-nwe/backend 7/7、deep-nesting
回归 exit 0）。

## 测试

- 新增 `crates/project-model/tests/bin_lib_cross_call.rs`：
  - bin→lib re-export 调用边存在性（断言 resolved 符号 id）
  - 非 target-root 文件 package 归属非 None
- 既有测试全绿：project-model 4+2、cli lib 48。

## 不在本轮的（显式出界）

- Fix G（质量门跨语言对齐，external_symbol_marking 补齐）：质量门 contract
  变化可能让存量项目从绿变红，独立卡处理。
- CLI `workspace` 子命令：并入 P2-P4 合并卡（`2026-08-25-polyglot-merge-
  and-alignment-execution-card.md`）的 P4，避免两次改 CLI/MCP 对称面。
- MCP 重连健壮性：非本仓缺陷（dsh-mcp-client rc.1）。

## 验收标准（达成情况）

1. ✅ stock-core `run → discover_existing` 边存在且指向
   `stock-core::crate::instance::discover_existing`；CALLS 9,357。
2. ✅ self-analysis（open-nwe/backend）CALLS 只增不减（4,475 → 62,452）。
3. ✅ 全量测试绿；新增回归测试通过；栈无回归。
4. ✅ docs-only diff riskLevel ≤ low。
5. ⏳ 装机版 promote 后 manifest serverVersion = 0.17.0-beta.2（release
   构建中，完成后执行）。
