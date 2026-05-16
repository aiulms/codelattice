# CodeLattice WebUI — Troubleshooting

## Port in use
```
OSError: [Errno 48] Address already in use
```
解决: `bash scripts/webui-runner.sh --port 8766`

## Snapshot generation failed
- 检查 root 路径存在
- 检查语言支持 (`rust/typescript/c/cpp/python/arkts/cangjie/auto`)
- 检查 `target/release/codelattice` 是否构建: `cargo build --release --bins`
- runner 返回 error + hint

## Root not found
runner 拒绝不存在或非目录路径。确保输入正确的绝对值或相对路径。

## Unsupported language
runner 支持: `auto, rust, typescript, c, cpp, python, arkts, cangjie`

## Empty graph
正常情况——小项目/特定语言 graph 节点较少。检查 `graph.summary.nodeCount`。

## File Mode vs Runner Mode
- **File Mode**: 直接打开 `index.html`，通过 file input 加载 JSON
- **Runner Mode**: 启动 `webui-runner.sh --open`，通过 API 生成/manage snapshots

切换方式：停止 runner 即为 File Mode；启动 runner 即为 Runner Mode。

## Report/Timeline empty state
- Report: 需要加载 snapshot 才能生成
- Timeline: 需要加载 2+ snapshots
- Diff: 需要加载 compare snapshot

## Path leaks
所有 fixture snapshots 使用 `--redact-root` 后不应含路径。如发现 leak，重新生成。

## Cleanup
```bash
rm -rf .codelattice-webui/        # 删除所有 runner 数据
```
