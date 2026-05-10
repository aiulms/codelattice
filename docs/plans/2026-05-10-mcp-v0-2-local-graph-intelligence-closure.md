# MCP v0.2 Closure Review — Local Graph Intelligence Pack

> **日期：** 2026-05-10
> **版本：** v0.2.0
> **状态：** Closed

---

## 一、目标

在 MCP v0.1 基础上新增 8 个本地图谱智能工具，为 AI agent 提供单仓库本地图查询能力（符号上下文、调用追踪、影响预览、图查询、项目概览、repo 注册状态、重命名预览）。

## 二、交付物

| 交付物 | 状态 |
|--------|------|
| `GraphView` 共享图查询层（HashMap-based in-memory） | Done |
| 8 个 v0.2 tool handlers | Done |
| tools_list() 更新至 16 tools | Done |
| tools/call dispatch 更新 | Done |
| 9 个新增集成测试（27/27 pass） | Done |
| dogfood script 更新（17/17 pass） | Done |
| contract doc 更新 | Done |
| README 更新 | Done |

## 三、工具清单

| # | 工具名 | 用途 |
|---|--------|------|
| 9 | `codelattice_symbol_context` | 符号丰富上下文：定义、边、诊断、confidence |
| 10 | `codelattice_calls_from` | 出边调用追踪（BFS） |
| 11 | `codelattice_calls_to` | 入边调用者追踪（反向 BFS） |
| 12 | `codelattice_impact_preview` | 变更影响范围预览（风险等级 + 受影响节点/文件） |
| 13 | `codelattice_query_graph` | 参数化图查询（nodeKind/edgeKind/name/file 过滤） |
| 14 | `codelattice_project_overview` | 项目综合概览（统计、质量、hotspots） |
| 15 | `codelattice_repo_registry` | Repo 注册状态（无持久化，每次重新分析） |
| 16 | `codelattice_rename_preview` | 重命名预览（applySupported=false，只读） |

## 四、设计决策

1. **GraphView struct**: 每次 tool call 构建一次，HashMap 索引 nodes_by_id / symbols_by_name / outgoing / incoming / diagnostics，避免重复解析。
2. **BFS traversal**: calls_from / calls_to / impact_preview 使用 BFS，max depth 3，limit 保护。
3. **Risk heuristic**: impact_preview — LOW (<=3 nodes, <=2 calls), MEDIUM (<=15 nodes, <=10 calls), HIGH (otherwise)。
4. **No process-local cache**: 每次 tool call 运行独立 analyze subprocess，无跨调用缓存。
5. **find_symbols**: exact match first → substring match (case-insensitive)。
6. **rename_preview**: applySupported=false — 只输出候选，不写文件。
7. **query_graph**: 只接受参数化过滤器，拒绝任意查询字符串。
8. **repo_registry**: 无持久化 registry，返回当前 root 的即时状态。

## 五、验证结果

- **Build**: Clean compile, `cargo fmt` applied
- **MCP tests**: 27/27 pass
- **bridge_roundtrip**: 13/13 pass
- **productization_commands**: 11/11 pass
- **Dogfood**: 17/17 pass (initialize + tools/list + 15 tool calls)

## 六、已知限制

- 无跨调用缓存，每次 tool call 运行完整 analyze subprocess
- BFS max depth 3，对于大型项目深度受限
- impact_preview risk heuristic 为简单计数阈值，非上下文感知
- query_graph 仅在 edgeKind 参数提供时返回 matched_edges
- repo_registry 无持久化状态
- rename_preview 不做 AST 安全校验

## 七、文件变更

- `crates/cli/src/mcp_server.rs`: GraphView struct + 8 tool handlers + tools_list + dispatch (~2270 lines)
- `crates/cli/tests/mcp_server.rs`: 9 new v0.2 tests (27 total)
- `scripts/mcp-dogfood.sh`: Updated to 16 tools + v0.2 tool calls
- `docs/architecture/mcp-v0-contract.md`: Updated to v0.2 with 8 new tool docs
- `README.md`: Updated MCP stdio entry
- `docs/plans/2026-05-10-mcp-v0-2-local-graph-intelligence-closure.md`: This file

## 八、Stop Lines Verified

- No default tool replacement
- No live repo writes
- No new dependencies
- No embeddings
- No Cypher parser
- No rename/refactor apply
- No cross-repo semantic edges
- All tools read-only
- Path deny list enforced
- /tmp output restriction on export_bridge
