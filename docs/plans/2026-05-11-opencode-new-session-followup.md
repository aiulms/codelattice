# CodeLattice MCP v0.6 — opencode New Session Followup

> **日期：** 2026-05-11
> **版本：** v0.6.0
> **状态：** Complete

---

## 一、目标

验证 opencode 新 session 能发现全部 21 个 CodeLattice MCP tools；修复 Cangjie `symbol_search` 返回 0 结果的问题；新增 `cache_prewarm` 工具提升首次调用体验。

---

## 二、变更清单

### 2.1 Cangjie symbol_search 修复

**问题**：`symbol_search(init)` 在 Cangjie 项目上返回 0 结果。

**根因**：Cangjie graph nodes 使用 `kind="symbol"` + `label="<display_name>"`，而旧代码按 `label == "symbol"` 过滤，排除了所有 Cangjie 符号。

**修复**：
- 过滤改用 `kind` 字段（symbol, function, method, class 等）
- Name 提取级联：`properties.name` → `label`（Cangjie）→ `id` 解析（Rust `::` + Cangjie `:` + `#arity` 去尾）
- File 提取级联：`sourcePath` → `manifestPath` → Cangjie `id` 格式解析

**验证**：`init` 搜索返回 10 matches。

### 2.2 cache_prewarm 新工具

**动机**：AI agent 打开项目后首次 tool call 触发完整分析（可能数秒），后续调用命中缓存。`cache_prewarm` 让 agent 主动预热，避免首个业务调用延迟。

**规格**：
- Input: `{ root, language?, strict? }`
- Output: `{ warmed, cacheHit, analysisDurationMs, summary: { symbolCount, nodeCount, edgeCount, sourceFileCount } }`
- `strict` 默认 `false`（与其他工具一致）
- 如果缓存已 fresh（mtime-valid），返回 `cacheHit=true`

### 2.3 opencode 真实客户端验证

- 新 opencode session 成功发现 21 个 tools
- 所有 21 tools 可正常调用（当前 session 的 binary 缺少 `tree-sitter-cangjie` feature，Cangjie 调用需重启后生效）

---

## 三、测试

- 52/52 MCP tests passing（49 existing + 3 new）
- 新增测试：
  - `mcp_cache_prewarm_warms_cache` — prewarm miss → project_overview cacheHit
  - `mcp_cache_prewarm_returns_hit_if_fresh` — second prewarm returns cacheHit=true
  - `mcp_cangjie_symbol_search_finds_init` — `#[cfg(feature = "tree-sitter-cangjie")]`，Cangjie fixture 搜索 "init"

---

## 四、脚本更新

- `mcp-dogfood.sh`: v0.6, 21 tools, step 21 prewarm
- `mcp-real-client-dry-run.sh`: >= 21 tools
- `install-mcp.sh`: docstring 21 tools

---

## 五、文档更新

- `mcp-v0-contract.md`: v0.6 changelog, §3.21 prewarm spec, §3.22 Cangjie fix notes
- `mcp-local-client-setup.md`: v0.5→v0.6, 20→21 tools, cache_prewarm 条目

---

## 六、关键决策

| 决策 | 理由 |
|------|------|
| `kind` 过滤而非 `label` | Cangjie `label` 是显示名，Rust `label` 是固定值 "symbol" |
| `strict=false` 默认 | 与大多数工具一致，仅 `analyze` 默认 `strict=true` |
| Name 提取级联策略 | 兼容 Rust 和 Cangjie 两种 node 结构 |
