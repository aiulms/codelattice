# AI Usability Pack A：ask one-hop / compact 去重 / job wait / TS 设计文档

> **状态**: Execution Card · **日期**: 2026-05-30

## Write Set

| 文件 | 改动 |
|------|------|
| `crates/cli/src/mcp_server.rs` | A(ask one-hop)、B(compact 去重)、C(job wait) |
| `crates/cli/src/mcp_job.rs` | C(job wait polling) |
| `docs/plans/2026-05-30-typescript-inprocess-job-runtime.md` | D(TS Pack B 设计) |

## Forbidden Set

- 不新增 MCP tool（默认 6，full 49）
- 不改 open-nwe / cangjie 等 live repo
- 不改 GitNexus-RC
- 不同步 CodeLattice-Tool
- 不做 destructive git 操作

## Stop-lines

- ask 执行只做轻量操作（symbol search / route detection），不阻塞做完整 call_chains
- job wait 最长 30s，不持有全局 busy 锁
- compact 去重只减少字段，不删除必要信息
