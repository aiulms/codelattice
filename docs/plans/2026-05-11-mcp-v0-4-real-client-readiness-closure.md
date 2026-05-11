# MCP v0.4 Real Client Readiness — Closure Review

> **日期：** 2026-05-11
> **版本：** v0.4.0
> **状态：** Complete

---

## 一、交付物

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/cli/src/mcp_server.rs` | 新增 `read_source_snippet()` 函数；`symbol_context` 新增 `sourceSnippet` 字段、`includeSnippet`/`snippetContext` 参数；wrapper 版本升至 0.4.0 |
| `crates/cli/tests/mcp_server.rs` | 新增 5 个 snippet 测试（正常输出、禁用、缓存命中、自定义上下文、候选全覆盖） |
| `scripts/install-mcp.sh` | **新增** — `--build`、`--print-config`、`--dry-run`，输出 Claude/Codex/opencode 配置片段 |
| `scripts/codelattice-mcp.sh` | 新增 `--self-test` 标志；版本升至 0.4.0 |
| `scripts/mcp-cache-smoke.sh` | **新增** — 4 项缓存行为验证（miss→hit、跨工具、clear→re-miss、snippet+cache） |
| `scripts/mcp-dogfood.sh` | 更新 snippet 验证检查 |

### 文档变更

| 文件 | 变更 |
|------|------|
| `docs/architecture/mcp-v0-contract.md` | §3.9 更新至 v0.4（sourceSnippet 字段、includeSnippet/snippetContext 参数）；版本号至 v0.4.0；变更历史 |
| `docs/architecture/mcp-local-client-setup.md` | 新增 §十 安装与自检；版本号至 v0.4.0 |

---

## 二、验收结果

| 检查项 | 结果 |
|--------|------|
| `cargo build` | ✅ 编译通过 |
| `cargo test -p gitnexus-rust-core-cli --test mcp_server` | ✅ 42/42 pass |
| `bash scripts/mcp-dogfood.sh` | ✅ 20/20 pass |
| `bash scripts/mcp-local-client-smoke.sh` | ✅ 9/9 pass（1 skip） |
| `bash scripts/mcp-cache-smoke.sh` | ✅ 4/4 pass |
| `bash scripts/codelattice-mcp.sh --self-test` | ✅ MCP handshake OK |
| source snippet in output | ✅ 包含 lines/startLine/endLine/totalLines |
| snippet 禁用 (`includeSnippet: false`) | ✅ 返回 null |
| snippet 缓存命中可用 | ✅ cacheHit=True 时仍有 snippet |
| snippet 文件不存在 | ✅ 返回 warning 结构，不 panic |
| install-mcp.sh --print-config | ✅ 输出 Claude/Codex/opencode 配置 |

---

## 三、Source Snippet 设计

| 设计选择 | 说明 |
|----------|------|
| 默认开启 | `includeSnippet` 默认 `true`，AI 客户端无需额外配置即可获得源码 |
| 可禁用 | `includeSnippet: false` 禁用，适用于只关心 graph 关系的场景 |
| 上下文行数 | 默认 3 行，最大 10 行，通过 `snippetContext` 控制 |
| 片段上限 | 硬编码 50 行，避免巨型函数导致 token 溢出 |
| 错误处理 | 文件不存在/不可读/空文件 → 结构化 warning，不 panic |
| 缓存兼容 | snippet 从文件系统实时读取（不缓存），确保始终反映磁盘状态 |

---

## 四、工具数量

工具总数不变：**18 个**。v0.4 不新增工具，而是增强 `codelattice_symbol_context` 的输出。

---

## 五、硬边界验证

| 约束 | 状态 |
|------|------|
| 不修改 GitNexus-RC runtime/schema/WebUI | ✅ 未触碰 |
| 不修改 GitNexus-RC-Tool dist | ✅ 未触碰 |
| 不碰 cangjie/open-nwe live repo | ✅ 未触碰 |
| 不切默认工具 | ✅ 未触碰 |
| 不自动改用户 AI 客户端配置 | ✅ install-mcp.sh 只打印 |
| 不新增依赖 | ✅ 仅用 std::fs |
| generatedAt 不参与 strict compare | ✅ 未改动 |

---

## 六、下一步 (v0.5 候选)

- 给 calls_from/calls_to 的 target/source 节点也加 snippet
- LRU eviction（当前缓存无大小限制）
- mtime-based cache invalidation
- cangjie snippet 支持（需 tree-sitter-cangjie feature）
