# CodeLattice External Reuse Pack Closure（2026-05-11）

## Scope

本轮目标是把 CodeLattice 从作者本机可用推进到外部 fresh clone 后可构建、可安装 MCP、可 smoke 的状态。变更只落在 CodeLattice repo：

- `scripts/install-mcp.sh`
- `scripts/promote-to-local-tool.sh`
- `scripts/codelattice-mcp.sh`
- `scripts/fresh-clone-smoke.sh`
- `README.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/plans/`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`

未修改 GitNexus-RC runtime/schema/WebUI、GitNexus-RC-Tool、live Cangjie repo、open-nwe、Codex/opencode/Claude 真实配置。

## Implementation Summary

### Portable install/runtime scripts

- `install-mcp.sh` 支持 `CODELATTICE_ROOT`、脚本位置自动推导 repo root、`CODELATTICE_TOOL_DIR`、`--install-dir <path>`。
- `install-mcp.sh --print-config` 改为使用当前选择的 stable wrapper 路径；stable runtime 不存在时给出 promote 提示。
- `install-mcp.sh --doctor` 区分 dev wrapper 和 stable wrapper；stable wrapper 缺失为 WARN，不阻断 fresh clone doctor。
- `promote-to-local-tool.sh` 支持 `CODELATTICE_ROOT`；无 `.git` 的 fresh-copy 场景下 `sourceCommit=unknown`，不阻断 promote。
- promoted manifest 新增 `layoutVersion` 与相对 `paths` 字段，保留 `sourceRepo`、`sourceCommit`、`sourceRemote`。
- `codelattice-mcp.sh` 明确为 contributor/debug 开发 wrapper；普通 AI client 推荐使用 promoted stable wrapper。

### Fresh clone smoke

新增 `scripts/fresh-clone-smoke.sh`：

- 默认用当前 repo 复制到 `/tmp/codelattice-fresh-smoke-*` 模拟 fresh clone，不联网 clone。
- 排除 `target/`、`.git/`、`.gitnexus/`、`.claude/`、`.agents/`、`CodeLattice-Tool/` 与临时 bridge JSON。
- 支持 `--keep-temp`、`--skip-tests`、`--install-dir <path>`。
- 不触碰真实 Codex/opencode/Claude 配置。
- fresh copy 无 `.git` 时，MCP server full test 中依赖 git repo 的 alpha tool-import smoke 不适合作为外部最小路径；脚本默认运行 focused MCP subset，并在输出中说明原因。
- 验证 promoted wrapper `--self-test`、`tools/list >= 21`、Rust portable fixture `project_overview`，并在 Cangjie fixture/feature 可用时跑 Cangjie fixture `project_overview`。

### Documentation externalization

- README 增加外部 fresh clone Quick Start、Alpha/daily-use candidate 状态、Rust+Cangjie 支持范围、CodeLattice 与 GitNexus-RC 的治理关系。
- README 说明 Cargo package/bin 仍叫 `gitnexus-rust-core-cli` 是兼容遗留，不影响使用。
- `docs/architecture/mcp-local-client-setup.md` 改为参数化 `$CODELATTICE_ROOT` / `$CODELATTICE_TOOL_DIR`，补充 fresh clone install path，并区分 stable wrapper 与 dev wrapper。
- `docs/plans/README.md` 更新 External Reuse Pack 索引。

### Cangjie project_overview compact regression

复现：Cangjie `project_overview` compact 顶层 `edgeCount/sourceFileCount/symbolCount/packageCount=0`，但 `summary` 和 `graph_overview` 正常。

Root cause：`handle_project_overview` / `GraphView::stats` 假定 Rust graph shape（node `label`、edge `type/source/target`），Cangjie graph 使用 node `kind`、edge `kind/sourceId/targetId`。compact mapper 没有优先使用 language-normalized `summary`。

修复：`handle_project_overview` 顶层计数改为优先使用 `result.summary`，缺失时才回退到 GraphView；top node/edge kinds 同时兼容 Rust 与 Cangjie 字段。未改变 schema。

新增回归：

- Rust `mcp_project_overview_rust` 断言 `edgeCount/sourceFileCount` 非零。
- Cangjie feature-gated `mcp_project_overview_cangjie_counts_are_nonzero` 断言 `symbolCount/sourceFileCount/edgeCount` 非零。

## Verification

已通过：

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test`
- `cargo test --features tree-sitter-cangjie`
- `scripts/install-mcp.sh --doctor`
- `scripts/codelattice-mcp.sh --self-test`
- `scripts/fresh-clone-smoke.sh`
- `scripts/fresh-clone-smoke.sh --skip-tests`
- `scripts/mcp-dogfood.sh`
- `scripts/mcp-real-client-dry-run.sh`
- `scripts/mcp-local-client-smoke.sh`

Smoke highlights：

- dogfood：22/22 PASS
- real client dry-run：10/10 PASS
- local client smoke：9 PASS / 0 FAIL / self-analysis optional skip
- fresh clone full path：promoted wrapper self-test PASS，tools/list = 21，Rust project_overview nonzero，Cangjie project_overview nonzero
- fresh clone `--skip-tests`：同样通过 build/promote/wrapper/tools/Rust+Cangjie fixture smoke

## GitNexus Detect Changes

命令：

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
```

结果：

- Changes：8 files, 31 symbols
- Affected processes：9
- Risk level：HIGH

HIGH 来源主要是 `handle_project_overview` 与 `run_script_with_timeout` 处于 MCP project overview / smoke 执行流中。已在执行前分别做 impact analysis；`run_script_with_timeout` impact 为 HIGH 时已暂停说明风险。全量 Rust、Cangjie feature、MCP server、fresh clone 与本地 MCP smokes 均已通过。

## Notes

- 本轮没有 promote 到 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`。
- 本轮没有修改真实 Codex/opencode/Claude 配置。
- simulated fresh clone 由于排除 `.git`，promote manifest 的 `sourceCommit` 会是 `unknown`；真实 clone 保留 `.git` 时会记录实际 commit。
- `.agents/` 为 agent 私有未跟踪目录，不纳入提交。

## Follow-ups

- Cargo package/bin 从 `gitnexus-rust-core-cli` 迁移到 CodeLattice 命名。
- 发布归档包或安装包，减少用户手动 promote 步骤。
- 组织外部 beta trial，收集不同机器上的 Cangjie parser / MCP client 兼容问题。
