# CodeLattice Rename Follow-up Closure

> **日期：** 2026-05-09
> **版本：** 1.0.0
> **类型：** Closure Review — rename 后残留扫描与收尾
> **前置：** [Local Rename and Index Refresh Closure](2026-05-09-codelattice-local-rename-and-index-refresh-closure.md)（commit `b953f69`）

---

## 一、背景

项目已从 `gitnexus-rust-core` 改名为 CodeLattice（本地路径 `/Users/jiangxuanyang/Desktop/codelattice`，GitCode remote `https://gitcode.com/aiulms/codelattice.git`）。本轮在 rename 后复核所有旧名残留，确保不会误导执行 AI。

---

## 二、扫描范围

在 codelattice 内搜索以下模式：

- `gitnexus-rust-core`（151 hits in 124 files）
- `GitNexus Rust Core` / `GitNexus Rust-Core`（0 hits）
- `/Users/jiangxuanyang/Desktop/gitnexus-rust-core`（0 hits in scripts，6 hits in old execution cards）
- `npx gitnexus`（17 hits，全部是"禁止使用"警告或历史事实）
- `alpha-trial-rust-smoke`（仅在 Tool registry 临时出现，cleanup 后消失）
- `rust-core bridge` / `Rust-core`（0 hits in scripts）

---

## 三、修复项

### 3.1 必须修复（已完成）

| 文件 | 修复内容 |
|------|---------|
| `docs/plans/2026-05-09-periodic-alpha-trial-run-001.md` | Target repo/path 从旧路径更新为 `/Users/jiangxuanyang/Desktop/codelattice`；旧路径标注为"旧路径"保留 |
| `docs/plans/2026-05-09-periodic-alpha-trial-run-001.md` | Summary table 中 `gitnexus-rust-core (self)` → `CodeLattice (self)` |
| `docs/plans/2026-05-09-periodic-alpha-trial-run-001.md` | Workspace state table 中 `gitnexus-rust-core` → `codelattice` |
| `docs/plans/2026-05-09-beta-readiness-go-no-go-review-001.md` | "gitnexus-rust-core 自身" → "CodeLattice 自身" |

### 3.2 合理保留（不修）

| 位置 | 保留原因 |
|------|---------|
| Cargo package name `gitnexus-rust-core-cli` | Cargo binary 兼容名，不改 |
| Cargo package name `gitnexus-project-model` | 内部 crate，不改 |
| Cargo package name `gitnexus-cangjie` | 内部 crate，不改 |
| `--format gitnexus-rc` | 下游消费格式兼容 flag，保留 |
| `--experimental-rust-core-bridge-graph` | Tool opt-in flag，保留 |
| `cargo run -p gitnexus-rust-core-cli -- ...` | 脚本中实际 cargo bin 名，保留 |
| `npx gitnexus` 在 command-authority.md | "禁止使用"规则说明，保留 |
| `npx gitnexus` 在 runbook / playbook | "不要这样做"警告，保留 |
| `npx gitnexus` 在 AGENTS.md | "Never use"规则，保留 |

### 3.3 历史事实保留（不修）

- 所有旧 execution card / closure review 中的旧路径、旧命令 — 编写时路径正确
- `docs/migration/from-gitnexus-rc.md` — 迁移文档
- `docs/plans/2026-05-09-legacy-naming-compatibility-cleanup-preflight.md` — 旧名清理策略文档
- `PROVENANCE.md` — 项目来源记录

### 3.4 Future cleanup（低优先级，本轮不做）

- 内部 architecture docs 中约 50+ 处 `gitnexus-rust-core` 用作描述性引用（不影响操作）
- `docs/smoke-targets-config.md`、`docs/architecture/consumer-contract.md` 中的旧名引用（功能性文档，不误导）

---

## 四、Tool Index 状态

- **Repo name:** `codelattice`
- **Indexed commit:** b953f69
- **Status:** ✅ up-to-date
- **detect-changes:** 2 files changed (本轮修改的 trial run-001 和 beta review-001)，low risk
- **Stale entries:** 无 `alpha-trial-*` 残留（cleanup 函数在 EXIT 时恢复 `codelattice` 索引）

---

## 五、alpha-trial-smoke.sh 副作用控制

- **Registry 临时名：** `alpha-trial-rust-smoke` / `alpha-trial-cangjie-smoke`
- **Cleanup：** trap EXIT 时 `--name "$RESTORE_REPO_NAME"` 重新索引为 `codelattice`
- **`.claude` / `CLAUDE.md`：** cleanup 函数中 `git ls-files` 检查后删除（不提交）
- **已知管道问题：** Tool 的 ANSI `[2K` 进度控制在 `set -euo pipefail` + `grep -q` 管道中偶尔匹配失败（pre-existing，非 rename 引起）。直接调用 Tool 时 `indexed successfully` 始终可见且 exit 0。

---

## 六、验证结果

| 检查 | 结果 |
|------|------|
| `cargo fmt --check` | ✅ Clean |
| `git diff --check` | ✅ Clean |
| `bash -n scripts/*.sh` | ✅ 3/3 syntax OK |
| Tool `status` | ✅ codelattice up-to-date |
| Tool `detect-changes --repo codelattice` | ✅ Low risk (docs only) |
| Tool `detect-changes --repo gitnexus-rc` | ✅ No changes detected |
| Tool `list` | ✅ codelattice registered, no stale alpha-trial entries |

---

## 七、未做事项

- Cargo package/binary 重命名（需兼容计划，不在本轮范围）
- 内部 architecture docs 大范围旧名替换（低优先级，不影响操作）
- `alpha-trial-smoke.sh` 的 grep 管道可靠性改善（pre-existing 问题，非 rename 相关）

---

## 八、结论

CodeLattice rename 后的高风险残留已全部清理。当前项目身份、路径、Tool index 均为 `codelattice`。旧名仅作为 Cargo 兼容名、下游兼容 flag、历史事实保留，不会误导执行 AI 的操作路径或项目身份判断。
