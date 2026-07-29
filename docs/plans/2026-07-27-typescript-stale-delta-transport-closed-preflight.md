# TypeScript stale delta `Transport closed` 修复 Preflight

> 日期：2026-07-27  
> 状态：执行卡已冻结  
> 类型：bugfix / 资源安全  
> 复现项目：`/Users/jiangxuanyang/Desktop/open-nwe/frontend`（只读）

## 1. 根因与证据

`open-nwe/frontend` 的正常 TypeScript 全量分析峰值约 20–50 MiB；MCP 在命中陈旧持久缓存后会走 stale delta 路径。该路径存在三处组合缺陷：

1. `scan_file_mtimes` 未跳过 `build` / `dist` / `out` / `coverage` 等生成目录；
2. delta 候选只按扩展名粗筛，不按请求语言隔离；
3. 所有 `.ts/.tsx/.js/.jsx` delta 文件会先送入 Rust `TreeSitterItemExtractor`，再送入 TS/JS 提取器。

当前 `open-nwe/frontend` 相对 2026-06-09 缓存新增 400 个 TS/JS 文件、约 32.5 MiB，其中包含两份 7.0 MiB Monaco worker 和两份 3.7 MiB Monaco API bundle。受控复现在 2.1 GiB RSS 主动终止，采样栈位于 Rust grammar 的 `ts_parser__recover`；未限制时 macOS Jetsam 记录约 64–68 GiB。

## 2. 影响评估

CodeLattice native impact preview：`scan_file_mtimes` 为 **MEDIUM** 风险，静态 blast radius 为 3 个文件；主要受影响域是 MCP 缓存新鲜度、增量符号覆盖和持久缓存命中路径。静态图未发现直接 caller，但源码中存在多处直接调用，必须以定向测试与 MCP 复现补足。

## 3. Execution Card

### Write Set

- `crates/cli/src/mcp_server.rs`
  - 缓存扫描目录过滤与分析器源文件发现对齐；
  - stale delta 候选按语言筛选；
  - Rust extractor 只接收 `.rs`。
- `crates/cli/tests/mcp_server.rs`
  - TypeScript stale cache 不得把 JS 当 Rust delta symbol。
- `crates/analysis-scheduler/src/lib.rs`、`crates/analysis-scheduler/tests/scheduler.rs`
  - scheduler fingerprint 同步排除常见生成目录，避免构建产物永久制造 stale cache。
- 本 preflight 与对应 closure review。

### Forbidden Set

- 不修改 `open-nwe` 源码或构建产物；
- 不删除或覆盖用户的 `~/.cache/codelattice`；
- 不修改 graph schema、CALLS 策略或语言提取器契约；
- 不通过提高 Codex timeout、提高机器内存或禁用所有缓存掩盖根因；
- 不做 destructive git 操作，不 commit/push。

### Stop-line

- TypeScript 正常源码被生成目录规则误排除；
- Rust stale delta 新增符号回归；
- 受控 `open-nwe` 复现超过 2 GiB RSS；
- `cargo fmt --check`、相关测试或 `git diff --check` 失败；
- native detect-changes 报告 high/critical 风险时停止并先告知用户。

## 4. 验证矩阵

1. RED：生成目录仍进入 stale scan；TypeScript stale delta 暴露伪 Rust symbol。
2. GREEN：上述两个回归测试通过，既有 Rust stale delta 测试通过。
3. 真实只读 smoke：使用隔离缓存复制旧 frontend cache，执行 `before_edit(useChat, execute=true)`；记录耗时、峰值 RSS、transport 状态。
4. 仓库门禁：`cargo fmt --check`、`git diff --check`、相关测试；闭环前运行 `scripts/codelattice-precommit-check.sh`。
