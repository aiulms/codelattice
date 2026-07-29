# TypeScript stale delta `Transport closed` 修复 Closure Review

> 日期：2026-07-27  
> 状态：源码修复、验证与本地 runtime promote 完成；未提交、未推送  
> 复现项目：`/Users/jiangxuanyang/Desktop/open-nwe/frontend`（只读）

## 1. 结论

`Transport closed` 是 CodeLattice MCP 进程被 macOS 杀死后的表象，不是 Codex timeout 或并发限制。陈旧 TypeScript 缓存的 delta 路径把新增 JS bundle 先交给 Rust tree-sitter grammar；错误恢复造成无界内存增长，最终收到 `SIGKILL`。

修复通过三层约束消除该路径：

1. MCP cache scan 与 scheduler fingerprint 跳过 `build`、`dist`、`out`、`.output`、`coverage`、`.cache` 等生成目录；
2. stale delta 按请求语言筛选源码扩展名；
3. Rust extractor 明确只接收 `.rs`，不再解析 TS/JS。

## 2. TDD 证据

生产代码修改前，以下测试均先观察到预期失败：

- `cache_stale_scan_ignores_generated_output_directories`：失败时 `build/main.js` 仍被跟踪；
- `typescript_stale_delta_does_not_parse_javascript_with_rust_grammar`：失败时 TypeScript 查询暴露 `fake_rust_delta_symbol`；
- `fingerprint_ignores_hidden_and_generated_directories`：失败时生成目录使 tracked file count 从 1 增至 4。

修复后上述测试全部通过，既有 `stale_file_added_uses_delta_and_background_refresh` 也通过，Rust 增量能力未回归。

## 3. `open-nwe` 受控复测

请求保持与故障场景一致：`codelattice_workflow(before_edit)`，root 为 `open-nwe/frontend`，language 为 `typescript`，symbol 为 `useChat`，`execute=true`。

| 实现 | 结果 | 耗时 | 峰值 RSS |
|---|---|---:|---:|
| 旧实现 + 陈旧缓存副本 | 触发 stop-line 后主动终止 | 9.45 s | > 2.1 GiB |
| 修复后 + 同一缓存副本 | 成功返回，无 JSON-RPC error | 1.424 s | 213.3 MiB |
| promoted release runtime + 同一缓存副本 | 成功返回，无 JSON-RPC error | 0.407 s | 202.9 MiB |

未受控的旧进程在 macOS Jetsam 报告中达到约 64–68 GiB。复测只复制旧缓存到临时目录；未修改 `open-nwe`，未删除或覆盖真实 `~/.cache/codelattice`。

## 4. 验证结果

- `cargo fmt --check`：通过；
- `git diff --check`：通过；
- 三个新增定向回归测试：通过；
- 既有 Rust stale delta 回归：通过；
- `cargo test`：通过；
- `scripts/codelattice-precommit-check.sh`：通过；
- MCP concurrency smoke：通过。

编译输出仍有仓库既有 warning，本次没有新增编译错误或测试失败。

## 5. 风险与交付边界

Native detect-changes 报告 `critical`：4 个 tracked 变更文件、10 个 changed symbols、16 个受影响 workspace projects、2 个 unsupported-language boundary hits。该等级主要来自共享 scheduler/MCP server 的跨项目静态扩散；因此依照 stop-line，本轮不 commit/push。

已按用户明确授权将 release runtime promote 到 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`；runtime doctor 显示 source commit `8ecc0de`，50 个工具和全部语言适配器自检通过。当前已断开的 MCP session 不会热恢复，仍需重启 Codex 或新建任务来加载新进程。
