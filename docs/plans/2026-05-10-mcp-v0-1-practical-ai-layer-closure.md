# MCP v0.1 Practical AI Layer — Closure Review

> **日期：** 2026-05-10
> **前置：** MCP v0 commit `7a8bc70`
> **状态：** ✅ COMPLETE

---

## 交付物

### 代码变更
| 文件 | 变更 |
|------|------|
| `crates/cli/src/mcp_server.rs` | 从 v0 扩展到 v0.1：新增 4 个 tool handlers、shared helper functions、unified error structure、output shaping |
| `crates/cli/tests/mcp_server.rs` | 从 10 测试扩展到 18 测试：8 个 v0.1 新测试 |
| `scripts/mcp-dogfood.sh` | NEW — dogfood 脚本，8/8 pass |

### 新增工具
| Tool | 实现方式 | 测试覆盖 |
|------|----------|----------|
| `codelattice_graph_overview` | run analyze subprocess → extract stats from JSON | ✅ graph_overview_rust |
| `codelattice_unresolved_report` | run analyze → filter CALLS edges by confidence/reason | ✅ unresolved_report_rust |
| `codelattice_symbol_search` | run analyze → search nodes by name substring | ✅ finds_helper, finds_main |
| `codelattice_export_bridge` | run analyze --format gitnexus-rc → write to /tmp | ✅ writes_to_tmp, rejects_non_tmp_path |

### 输出整形
- `codelattice_analyze`: 默认 compact（graph removed），includeGraph=true 时包含。测试：compact_excludes_graph, include_graph_returns_graph
- `codelattice_quality`: failed gates 排前面
- `codelattice_smoke`: 失败时增加 hint
- 错误结构：code + message + details? + hint?

### 文档
| 文件 | 状态 |
|------|------|
| `docs/architecture/mcp-v0-contract.md` | 更新为 v0.1（8 tools, new error codes, new safety rules） |
| `docs/plans/2026-05-10-mcp-v0-1-practical-ai-layer-preflight.md` | NEW |
| `docs/plans/2026-05-10-mcp-v0-1-dogfood-report.md` | NEW |
| `docs/plans/2026-05-10-mcp-v0-1-practical-ai-layer-closure.md` | NEW (this file) |
| `docs/plans/README.md` | Updated with v0.1 entry |

## 验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo build` | ✅ clean, 0 warnings |
| `cargo fmt --check` | ✅ clean |
| `git diff --check` | ✅ clean |
| MCP tests (18) | ✅ 18/18 pass |
| bridge_roundtrip (13) | ✅ 13/13 pass |
| productization_commands (11) | ✅ 11/11 pass |
| full cargo test | ✅ 0 failures |
| alpha smoke --rust-only | ✅ 5 pass, 0 fail, 1 skip |
| dogfood (8 checks) | ✅ 8/8 pass |
| Tool index refresh | ✅ indexed 4,498 nodes / 7,869 edges |

## 新增依赖
**无**。所有新功能复用现有 serde_json + std::io + std::process。

## Safety Guard
- Path deny list: `/Users/jiangxuanyang/Desktop/cangjie` 阻止
- Output path restriction: export_bridge 仅允许 /tmp
- Cangjie unsupported: unresolved_report 返回 supported=false 而非伪造数据
- Error structure: 所有可区分错误类型有独立 code

## 硬边界确认
- ✅ 未切默认工具
- ✅ 未修改 GitNexus-RC runtime / schema / WebUI
- ✅ 未修改 Tool dist（仅只读调用做 index refresh）
- ✅ 未修改 live repo
- ✅ 未放宽 bridge adapter validator
- ✅ 未重命名 Cargo package / binary
- ✅ 未新增依赖
- ✅ MCP server 仍为 read-only（export_bridge 仅写 /tmp）
- ✅ generatedAt 不参与 deterministic compare

## 下一步建议
- MCP v0.2 可考虑：SSE transport（远程使用）、production_assist_dry_run（安全写操作预演）、symbol_context（符号上下文展开）
- Tool index 需 `--force` re-analyze 更新到本次 commit
- Cangjie unresolved_report 可在后续版本添加 CALLS edge confidence 分类后实现
