# MCP Local Client Setup — CodeLattice Sidecar Server

> **日期：** 2026-05-11
> **版本：** v0.5.0
> **状态：** Active

---

## 一、定位

CodeLattice MCP server 是一个 **sidecar server**，为 AI 编程工具提供本地单仓库图谱智能：

- **与 GitNexus MCP 并存**，不替代
- **Read-only** — 只读分析，不写源码
- **Rust / Cangjie only** — 仅支持这两种语言
- **单仓库** — 每次 tool call 针对一个 root
- **无持久化** — 不做 graph 存储、repo 注册

### 何时用 CodeLattice MCP vs GitNexus MCP

| 场景 | 推荐 |
|------|------|
| Rust/Cangjie 项目结构、symbol、calls、quality | CodeLattice MCP |
| production detect-changes / impact | GitNexus MCP / Tool |
| legacy graph / multi-repo / cross-repo | GitNexus MCP / Tool |
| 快速本地质量检查 | CodeLattice MCP |
| 真实重命名 / refactoring apply | IDE / language server（非 MCP） |

---

## 二、启动命令

```bash
bash /Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh
```

启动后进入 JSON-RPC over stdio 模式。日志输出到 stderr，stdout 为纯净 JSON-RPC。

### 环境变量

| 变量 | 用途 | 默认 |
|------|------|------|
| `CODELATTICE_ROOT` | CodeLattice 源码根目录 | 自动从脚本位置检测 |
| `CODELATTICE_MCP_BIN` | 预构建 binary 路径 | 自动选择 release → debug → cargo run |
| `CODELATTICE_LOG_LEVEL` | 日志级别（保留，当前未使用） | — |

---

## 三、Codex 配置示例

> ⚠️ 以下为示例，不修改真实 `~/.codex/config.toml`

```toml
# ~/.codex/config.toml (示例)
[mcp_servers.codelattice]
command = "bash"
args = ["/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh"]

# 如果使用预构建 binary：
# [mcp_servers.codelattice]
# command = "/Users/jiangxuanyang/Desktop/codelattice/target/release/gitnexus-rust-core-cli"
# args = ["mcp"]
```

---

## 四、Claude Desktop / Claude Code 配置示例

> ⚠️ 以下为示例，不修改真实 `claude_desktop_config.json`

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh"]
    }
  }
}
```

Claude Code (CLI) 配置路径：`~/.claude/claude_desktop_config.json` 或项目级 `.claude/config.json`。

如需与 GitNexus MCP 并存：

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["/Users/jiangxuanyang/Desktop/codelattice/scripts/codelattice-mcp.sh"]
    },
    "gitnexus": {
      "command": "node",
      "args": ["/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js"]
    }
  }
}
```

---

## 五、opencode 配置示例

> ⚠️ 以下为示例。实际配置路径：`~/.config/opencode/opencode.json`

opencode 使用 `mcp` 字段配置 MCP servers，格式与 Codex / Claude Desktop 不同：

```json
{
  "mcp": {
    "gitnexus": {
      "type": "local",
      "command": [
        "node",
        "/Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js",
        "mcp"
      ],
      "enabled": true
    },
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

1. CodeLattice 作为 sidecar 接入，不替代 GitNexus
2. 使用 wrapper 脚本而非直接 `cargo run`
3. 配置后需重启 opencode session 才能发现新 tools
4. wrapper 会自动检测 release / debug binary

---

## 六、20 个 MCP 工具一览

| # | 工具名 | 用途 | 版本 |
|---|--------|------|------|
| 1 | `codelattice_analyze` | 完整分析（graph summary + quality + diagnostics） | v0 |
| 2 | `codelattice_quality` | 质量门检查 | v0 |
| 3 | `codelattice_summary` | 紧凑 stats summary | v0 |
| 4 | `codelattice_smoke` | 端到端 smoke 测试 | v0 |
| 5 | `codelattice_graph_overview` | 图概览（按 kind 分组） | v0.1 |
| 6 | `codelattice_unresolved_report` | 未解析调用/诊断报告 | v0.1 |
| 7 | `codelattice_symbol_search` | 符号搜索 | v0.1 |
| 8 | `codelattice_export_bridge` | 导出 GitNexus-RC bridge JSON | v0.1 |
| 9 | `codelattice_symbol_context` | 符号丰富上下文 | v0.2 |
| 10 | `codelattice_calls_from` | 出边调用追踪（BFS） | v0.2 |
| 11 | `codelattice_calls_to` | 入边调用者追踪（BFS） | v0.2 |
| 12 | `codelattice_impact_preview` | 变更影响预览（风险等级） | v0.2 |
| 13 | `codelattice_query_graph` | 参数化图查询 | v0.2 |
| 14 | `codelattice_project_overview` | 项目综合概览 | v0.2 |
| 15 | `codelattice_repo_registry` | Repo 注册状态 | v0.2 |
| 16 | `codelattice_rename_preview` | 重命名预览（只读） | v0.2 |
| 17 | `codelattice_cache_status` | 查询进程内分析缓存状态 | v0.3 |
| 18 | `codelattice_cache_clear` | 清空进程内分析缓存 | v0.3 |
| 19 | `codelattice_production_assist` | 生产就绪检查（dry-run） | v0.5 |
| 20 | `codelattice_compare_runs` | 对比两次分析结果 | v0.5 |

---

## 七、推荐使用策略

### 首次打开项目

```
1. codelattice_project_overview  → 了解项目结构、规模、质量
2. codelattice_quality           → 检查质量门
```

### 理解代码

```
3. codelattice_symbol_context    → 查看某个符号的完整上下文
4. codelattice_calls_from        → 追踪某个函数调用了谁
5. codelattice_calls_to          → 追踪谁调用了某个函数
```

### 评估变更影响

```
6. codelattice_impact_preview    → 预览改动影响范围
7. codelattice_rename_preview    → 预览重命名影响（只读）
```

### 通用查询

```
8. codelattice_symbol_search     → 搜索符号
9. codelattice_query_graph       → 按 kind/name/file 过滤
10. codelattice_graph_overview   → 获取图概览
```

---

## 八、安全说明

1. **Read-only**: 所有 16 个工具只读项目源码，不修改任何文件
2. **Live repo deny**: `/Users/jiangxuanyang/Desktop/cangjie` 等生产 live repo 默认拒绝
3. **No default switch**: CodeLattice MCP 不会修改任何默认工具配置
4. **Temp files only**: `export_bridge` 仅写入 /tmp
5. **No rename apply**: `rename_preview` 返回 `applySupported: false`
6. **No arbitrary queries**: `query_graph` 只接受参数化过滤器

---

## 九、Troubleshooting

### cargo not found

```
ERROR: cargo run failed
```

确保 `cargo` 在 PATH 中。如果是通过 GUI client 启动（Claude Desktop），PATH 可能不包含 Cargo：
- 在 wrapper 前添加 `export PATH="$HOME/.cargo/bin:$PATH"`
- 或设置 `CODELATTICE_MCP_BIN` 指向预构建 binary

### path denied

```json
{"error": "path_denied", "message": "Root path is on the deny list"}
```

root 路径在 deny list 中（如 live cangjie repo）。请使用其他项目路径或调整 deny list。

### feature disabled

```
ERROR: Cangjie feature not compiled
```

Cangjie 支持需要编译时启用 feature：
```bash
cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie
```

### timeout

```json
{"error": "timeout", "message": "Subprocess exceeded time limit"}
```

大型项目分析可能超时。默认 timeout 足够大多数 Rust/Cangjie 项目。如果是超大项目，可先 `cargo build` 减少首次分析延迟。

### stale Tool index

如果 Tool index 与代码不同步：
```bash
node /path/to/gitnexus/dist/cli/index.js analyze /path/to/project --force --skip-agents-md --name project
```

### old binary name

二进制仍叫 `gitnexus-rust-core-cli`（旧工作名），MCP server 已重命名为 "codelattice"（server name），但 binary 路径未重命名。这是已知遗留，不影响功能。

---

## 十、安装与自检 (v0.4 新增, v0.5 增强)

### install-mcp.sh

```bash
# 构建 release binary
bash scripts/install-mcp.sh --build

# 打印可复制的客户端配置片段
bash scripts/install-mcp.sh --print-config

# 仅显示会做什么（不实际构建）
bash scripts/install-mcp.sh --build --dry-run

# 健康检查 (v0.5 新增)
bash scripts/install-mcp.sh --doctor
```

该脚本**不会自动修改**任何客户端配置文件。它只输出可复制粘贴的 JSON/TOML 片段。

`--doctor` 检查：binary、wrapper、MCP handshake、tools/list (>= 20)、cache_status (maxEntries)。

### codelattice-mcp.sh --self-test

```bash
bash scripts/codelattice-mcp.sh --self-test
```

验证：
1. CODELATTICE_ROOT 有效
2. Binary 可找到且可执行
3. MCP handshake 成功（initialize → 返回 codelattice server info）
4. tools/list 返回 >= 20 个工具 (v0.5 新增)
5. cache_status 包含 maxEntries 和 totalEvictions (v0.5 新增)

### mcp-cache-smoke.sh

```bash
bash scripts/mcp-cache-smoke.sh
```

验证缓存行为：
1. Miss → Hit（同一工具连续调用）
2. Cross-tool cache reuse
3. cache_clear 后重新 miss
4. 缓存命中时源码片段仍然可用

### mcp-real-client-dry-run.sh (v0.5 新增)

```bash
bash scripts/mcp-real-client-dry-run.sh [root_dir]
```

模拟真实 MCP 客户端调用 10 个高频工具，不修改任何配置：
1. initialize handshake
2. tools/list (20 tools)
3. cache_status (empty)
4. codelattice_analyze (miss)
5. codelattice_graph_overview
6. codelattice_symbol_context
7. codelattice_calls_from
8. codelattice_impact_preview
9. codelattice_production_assist
10. cache_status (populated)
