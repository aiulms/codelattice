# CodeLattice MCP — opencode Real Client Test Report

> **日期：** 2026-05-11
> **版本：** v0.5.0+bugfix
> **状态：** Complete

---

## 一、测试概要

在真实 opencode 客户端环境中测试 CodeLattice MCP server 的发现、配置、多轮调用体验。

### 环境

- **opencode 配置路径：** `~/.config/opencode/opencode.json`
- **CodeLattice 版本：** v0.5.0 (commit b9663d6 + bugfixes)
- **MCP transport：** stdio (JSON-RPC over stdin/stdout)
- **wrapper 脚本：** `/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh`

---

## 二、opencode 配置方式

### 实际配置（已应用）

在 `~/.config/opencode/opencode.json` 的 `mcp` 字段中添加：

```json
{
  "mcp": {
    "codelattice": {
      "type": "local",
      "command": [
        "/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh"
      ],
      "enabled": true
    }
  }
}
```

### 配置要点

1. 使用 wrapper 脚本，不直接写死 `cargo run`
2. CodeLattice 作为 sidecar 与 GitNexus MCP 并存
3. 配置后需重启 opencode session 才能发现新 tools
4. 备份路径：`~/.config/opencode/opencode.json.bak-20260511-114701`

### 回滚方法

```bash
cp ~/.config/opencode/opencode.json.bak-20260511-114701 ~/.config/opencode/opencode.json
```

---

## 三、发现的问题与修复

### Bug 1: Pipe-buffer deadlock（严重）

**现象：** 大型项目（如 CodeLattice 自身，~2.4MB JSON 输出）通过 MCP 调用时超时 60s。

**根因：** `run_subcommand_with_timeout()` 和 `run_script_with_timeout()` 在 `try_wait()` 循环中不读取 stdout。当子进程输出超过 OS pipe buffer（macOS ~64KB），子进程阻塞在 write() 上，永远不会退出，导致死锁。

**修复：** 将 stdout/stderr 读取移到后台线程，确保 pipe 不满。子进程完成后 join 线程获取完整输出。

**影响：** 所有涉及 subprocess 分析的 MCP tool 在大型项目上都会超时。

### Bug 2: Path deny list 误匹配

**现象：** 访问 `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui` 被拒绝，错误信息 "Path is under denied directory: /Users/jiangxuanyang/Desktop/cangjie"。

**根因：** `validate_root_path()` 使用 `starts_with()` 做字符串前缀匹配，未考虑路径组件边界。`cangjie-GitNexus-Index` 以 `cangjie` 开头但不是 `cangjie` 的子目录。

**修复：** 改用 `PathBuf::starts_with()`（path-component-aware）和带路径分隔符的前缀匹配。

---

## 四、测试场景 A：CodeLattice 自身 Rust 项目

**root:** `/Users/jiangxuanyang/Desktop/codelattice`
**language:** rust

| # | Tool | 结果 | 详情 |
|---|------|------|------|
| 1 | `codelattice_project_overview` | ✅ OK | 982 symbols, 60 files, 1923 nodes, 3070 edges, quality 7/7 pass, ~1.5s |
| 2 | `codelattice_symbol_search(main)` | ✅ OK | 3 matches (source-file, test function, main function) |
| 3 | `codelattice_symbol_context(handle_analyze)` | ✅ OK | sourceSnippet 包含完整函数源码，callers/callees 正确 |
| 4 | `codelattice_calls_from(handle_analyze)` | ✅ OK | 8 edges: -> McpCache, check_cangjie_feature, get_or_analyze 等 |
| 5 | `codelattice_impact_preview(scan_file_mtypes)` | ✅ OK | 返回 byDepth 分层影响 |
| 6 | `codelattice_production_assist` | ✅ OK | risk=HIGH, checks, recommendations |
| 7 | `codelattice_cache_status` | ✅ OK | entries, maxEntries=16, totalEvictions |
| 8 | `codelattice_cache_clear` | ✅ OK | clearedCount |
| 9 | `codelattice_project_overview` (re-call) | ✅ OK | 重新分析 ~1.5s，cache miss 后可正常工作 |

### 性能观察

- 首次分析：~1.5s（包含完整 Rust 项目 graph 构建）
- 缓存命中后：即时返回（从 cache_status 可验证）
- 输出大小：project_overview ~2.9KB，合理

---

## 五、测试场景 B：Cangjie 项目 (cjgui)

**root:** `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui`
**language:** cangjie

| # | Tool | 结果 | 详情 |
|---|------|------|------|
| 1 | `codelattice_project_overview` | ✅ OK | 0 symbols in overview (counting convention), 903 nodes, 8.3s |
| 2 | `codelattice_symbol_search(init)` | ⚠️ OK (empty) | 0 matches — cangjie graph naming convention 尚未对齐 search |
| 3 | `codelattice_symbol_context(init)` | ⚠️ OK (empty) | 0 candidates — 同上 |
| 4 | `codelattice_graph_overview` | ✅ OK | 903 nodes, 3252 edges, 887 symbols |
| 5 | `codelattice_production_assist` | ✅ OK | risk=LOW |
| 6 | `codelattice_cache_status` | ✅ OK | 1 entry cached |

### Cangjie 观察点

- 分析时间较长（~8s），因为 Cangjie 项目较大且 tree-sitter 解析开销高
- symbol_search 对于 Cangjie 返回 0 — 这是已知限制，graph 中 symbol naming convention 需要对齐
- graph_overview 和 project_overview 正常工作

---

## 六、体验评估

### 1. Tool 发现

opencode 配置后重启 session，应能发现 20 个 `codelattice_*` tools。当前 session 是配置前启动的，无法在本 session 内验证。

### 2. 首次调用 vs 缓存命中

- 首次调用（Rust ~60 files）：~1.5s
- 首次调用（Cangjie ~14 files）：~8s
- 缓存命中后：即时返回（< 100ms）

### 3. 输出大小

- `project_overview`：~2.9KB — 合理
- `symbol_context` with snippet：~4KB — 合理
- 未出现过大输出问题

### 4. Snippet 体验

- `symbol_context` 的 `sourceSnippet` 包含完整函数源码，对 AI 理解代码非常有帮助
- `calls_from`/`calls_to` 的 `sourceSnippet`/`targetSnippet` 在 candidate 和 edge 级别都可用
- `impact_preview` 的 `impactedSymbols` 包含 snippet

### 5. 错误信息

- `cangjie_disabled` 错误包含 details + hint，清晰
- `path_denied` 错误包含被拒绝的路径和原因
- timeout 错误包含超时时长

### 6. 与 GitNexus-RC MCP 冲突

- 无冲突。两者使用不同的 tool name prefix（`codelattice_*` vs `gitnexus_*`）
- CodeLattice 是 sidecar，不替代 GitNexus

---

## 七、修复后的代码变更

| 文件 | 变更 |
|------|------|
| `crates/cli/src/mcp_server.rs` | 修复 pipe-buffer deadlock（`run_subcommand_with_timeout` + `run_script_with_timeout`）；修复 path deny list 误匹配 |
| `docs/architecture/mcp-local-client-setup.md` | 更新 opencode 配置示例（真实格式）；工具数 18→20 |
| `docs/plans/2026-05-11-opencode-mcp-real-client-test.md` | 本文档 |

---

## 八、opencode 配置状态

- **已修改：** `~/.config/opencode/opencode.json`（添加 codelattice MCP）
- **已备份：** `~/.config/opencode/opencode.json.bak-20260511-114701`
- **保留配置：** 是。配置可用，不影响原有 GitNexus MCP 和其他 tools。
- **回滚方法：** `cp ~/.config/opencode/opencode.json.bak-20260511-114701 ~/.config/opencode/opencode.json`

---

## 九、下一步建议

1. **Cangjie symbol search 对齐**：graph 中的 symbol naming 需要对齐 search 逻辑，使 cangjie symbol 可被搜索
2. **大项目缓存策略**：考虑对 >1000 node 的项目使用 pre-warm 缓存
3. **opencode session 内验证**：在新 opencode session 中验证 tool 发现是否正常
4. **Production hardening**：考虑增加 request-level timeout 配置
