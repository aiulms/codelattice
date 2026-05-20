# MCP Local Client Setup — CodeLattice Sidecar Server

> **日期：** 2026-05-11
> **版本：** v0.7.0
> **状态：** Active

---

## 一、定位

CodeLattice MCP server 是一个 **sidecar server**，为 AI 编程工具提供本地代码图谱智能：

- **CodeLattice-native daily workflow** — 日常 analyze / impact / detect-changes 优先使用 CodeLattice
- **Read-only** — 只读分析，不写源码
- **多语言** — Rust / Cangjie / ArkTS / TypeScript / C / C++ / Python / Shell
- **单仓库 + workspace** — 单项目分析与 workspace 图谱/跨项目影响均支持
- **静态分析** — 不执行目标项目代码，不证明运行时或覆盖率

### 何时用 CodeLattice MCP vs GitNexus MCP

| 场景 | 推荐 |
|------|------|
| 支持语言的项目结构、symbol、calls、quality | CodeLattice MCP |
| CodeLattice 日常 detect-changes / impact | CodeLattice CLI / MCP |
| workspace graph / cross-project impact | CodeLattice MCP |
| 历史 GitNexus process-flow 对照 | GitNexus MCP / Tool（fallback） |
| 快速本地质量检查 | CodeLattice MCP |
| 真实重命名 / refactoring apply | IDE / language server（非 MCP） |

---

## 二、启动命令

### 推荐：稳定运行目录

日常 AI IDE（Codex / opencode / Claude）应指向稳定运行目录，而不是开发
checkout：

```bash
export CODELATTICE_TOOL_DIR="$HOME/Desktop/CodeLattice-Tool"
bash "$CODELATTICE_TOOL_DIR/codelattice-mcp.sh"
```

启动后进入 JSON-RPC over stdio 模式。日志输出到 stderr，stdout 为纯净 JSON-RPC。

### Fresh clone 安装路径

外部用户从 fresh clone 到 MCP 可用的最小路径：

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

bash scripts/install-mcp.sh --build

export CODELATTICE_TOOL_DIR="$HOME/Desktop/CodeLattice-Tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test

bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

这样开发 checkout 的源码修改、debug rebuild、wrapper 变更不会影响新启动的 AI
IDE；只有再次运行 promote 才会更新稳定运行版。

### 开发调试入口

仅开发/调试 CodeLattice 本身时使用：

```bash
export CODELATTICE_ROOT=/path/to/codelattice
bash "$CODELATTICE_ROOT/scripts/codelattice-mcp.sh"
```

### 环境变量

| 变量 | 用途 | 默认 |
|------|------|------|
| `CODELATTICE_ROOT` | CodeLattice 源码根目录 | 自动从脚本位置检测 |
| `CODELATTICE_TOOL_DIR` | promoted stable runtime 目录 | `$HOME/Desktop/CodeLattice-Tool` |
| `CODELATTICE_MCP_BIN` | 预构建 binary 路径 | 自动选择 release → debug → cargo run |
| `CODELATTICE_LOG_LEVEL` | 日志级别（保留，当前未使用） | — |

---

## 三、Codex 配置示例

> ⚠️ 以下为示例，不修改真实 `~/.codex/config.toml`

```toml
# ~/.codex/config.toml (示例)
[mcp_servers.codelattice]
type = "stdio"
command = "bash"
args = ["/path/to/CodeLattice-Tool/codelattice-mcp.sh"]
```

开发调试可临时指向 checkout wrapper，但不建议作为日常 AI IDE 配置：

```toml
[mcp_servers.codelattice]
type = "stdio"
command = "bash"
args = ["/path/to/codelattice/scripts/codelattice-mcp.sh"]
```

---

## 四、Claude Desktop / Claude Code 配置示例

> ⚠️ 以下为示例，不修改真实 `claude_desktop_config.json`

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["/path/to/CodeLattice-Tool/codelattice-mcp.sh"]
    }
  }
}
```

Claude Code (CLI) 配置路径：`~/.claude/claude_desktop_config.json` 或项目级 `.claude/config.json`。

如需与 legacy GitNexus MCP 并存（仅作 fallback / 历史 process-flow 对照）：

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "bash",
      "args": ["/path/to/CodeLattice-Tool/codelattice-mcp.sh"]
    },
    "gitnexus": {
      "command": "node",
      "args": ["/path/to/GitNexus-RC-Tool/gitnexus/dist/cli/index.js", "mcp"]
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
        "/path/to/GitNexus-RC-Tool/gitnexus/dist/cli/index.js",
        "mcp"
      ],
      "enabled": true
    },
    "codelattice": {
      "type": "local",
      "command": [
        "/path/to/CodeLattice-Tool/codelattice-mcp.sh"
      ],
      "enabled": true
    }
  }
}
```

### 配置要点

1. CodeLattice 是日常本地 sidecar；GitNexus 仅作 legacy fallback / 对照
2. 使用 wrapper 脚本而非直接 `cargo run`
3. 配置后需重启 opencode session 才能发现新 tools
4. wrapper 使用已 promote 的稳定 release binary；开发区变更不会自动影响 AI IDE

---

## 六、21 个 MCP 工具一览

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
| 21 | `codelattice_cache_prewarm` | 预热进程内分析缓存 | v0.6 |

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

1. **Read-only**: 所有 24 个工具只读项目源码，不修改任何文件
2. **Live repo deny with exemptions**: 配置为 live Cangjie 源码根的路径默认拒绝，但 `runtime/cjgui` 子路径可明确豁免用于只读分析
3. **No default switch**: CodeLattice MCP 不会修改任何默认工具配置
4. **Temp files only**: `export_bridge` 仅写入 /tmp
5. **No rename apply**: `rename_preview` 返回 `applySupported: false`
6. **No arbitrary queries**: `query_graph` 只接受参数化过滤器
7. **No source modification to live repos**: 所有 cangjie live 分析均为只读

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

可选语言支持需要编译时启用对应 feature：
```bash
cargo build -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python --bins
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

### cangjie-live-codelattice (v0.8 新增)

CodeLattice 产出的 live 仓颉图已注册到 GitNexus-RC-Tool registry，名称为 `cangjie-live-codelattice`：

```bash
# 查看 registry
node /path/to/gitnexus/dist/cli/index.js list

# 查询符号
node /path/to/gitnexus/dist/cli/index.js context init -r cangjie-live-codelattice

# 刷新分析（先产出 bridge JSON，再导入）
bash scripts/cangjie-live-codelattice-smoke.sh --analyze
bash scripts/cangjie-live-codelattice-smoke.sh --tool-ingest
```

**不再推荐使用裸 `cjgui`**。旧 `cjgui` entries 仅保留历史兼容。生产分析推荐 `cangjie-live-codelattice`，测试 fixture 推荐 `cjgui-index`。

### cangjie-production-alias-check.sh (v0.8 新增)

检查 live repo 是否处于 stable window：

```bash
# 查看状态
bash scripts/cangjie-production-alias-check.sh --status

# 运行 MCP smoke
bash scripts/cangjie-production-alias-check.sh --smoke

# 完整生产流水线
bash scripts/cangjie-production-alias-check.sh --full
```

Stable window 规则：
- dirty ≤ 10: **GREEN** — 安全执行 full smoke + 默认切换评估
- dirty 11-50: **YELLOW** — 只读 analyze/mcp，不建议切默认
- dirty > 50: **RED** — 建议等待

详见 `docs/plans/2026-05-11-cangjie-production-alias-switch-plan.md`。

### compatibility binary name

主二进制现在是 `codelattice`。`gitnexus-rust-core-cli` 作为旧工作名兼容 binary 继续构建，旧脚本可逐步迁移。

---

## 十、安装与自检 (v0.4 新增, v0.5 增强)

### install-mcp.sh

```bash
# 构建 release binary
bash scripts/install-mcp.sh --build

# 打印可复制的客户端配置片段
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config

# 仅显示会做什么（不实际构建）
bash scripts/install-mcp.sh --build --dry-run

# 健康检查 (v0.5 新增)
bash scripts/install-mcp.sh --doctor
```

该脚本**不会自动修改**任何客户端配置文件。它只输出可复制粘贴的 JSON/TOML 片段。

`--doctor` 检查：binary、开发 wrapper、stable wrapper 状态、MCP handshake、full toolset tools/list (>= 51)、cache_status、language support profile、fixture-level smoke。默认客户端不设置环境变量时使用 AI toolset，只暴露 facade-first 入口；调试/回归脚本会显式设置 `CODELATTICE_MCP_TOOLSET=full`。

### promote-to-local-tool.sh (runtime isolation)

```bash
export CODELATTICE_TOOL_DIR="$HOME/Desktop/CodeLattice-Tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
```

将当前已验证的 CodeLattice 构建提升到稳定运行目录：

```text
$CODELATTICE_TOOL_DIR
```

生成：

```text
CodeLattice-Tool/
  codelattice-mcp.sh
  manifest.json
  bin/
    codelattice
    codelattice-cli
    gitnexus-rust-core-cli
```

Codex / opencode / Claude 等 AI 客户端应指向参数化稳定 wrapper，例如 `/path/to/CodeLattice-Tool/codelattice-mcp.sh`。
这避免开发 checkout 的临时改动影响正在使用的 AI IDE。

### codelattice-mcp.sh --self-test

```bash
bash scripts/codelattice-mcp.sh --self-test
```

验证：
1. CODELATTICE_ROOT 有效
2. Binary 可找到且可执行
3. MCP handshake 成功（initialize → 返回 codelattice server info）
4. 默认 tools/list 返回 AI facade-first 小工具面；`CODELATTICE_MCP_TOOLSET=full` 返回 >= 51 个工具
5. 每个 tool definition 都带 `annotations` 和 `x-codelattice-permissionProfile`，用于说明只读、cache 写入、`/tmp` artifact 写入、项目代码执行和网络访问预期
5. cache_status 包含 maxEntries 和 totalEvictions (v0.5 新增)
6. cangjieSupport / arktsSupport / typescriptSupport / cSupport / cppSupport / pythonSupport 检测

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
2. tools/list（默认 AI 小工具面；full 模式 51 tools）
3. cache_status (empty)
4. codelattice_analyze (miss)
5. codelattice_graph_overview
6. codelattice_symbol_context
7. codelattice_calls_from
8. codelattice_impact_preview
9. codelattice_production_assist
10. cache_status (populated)

---

## 十一、Profile 与 Cangjie 支持 (v0.7 新增)

### Profile 检测

MCP server 的 `initialize` 响应包含 profile 信息：

```json
{
  "serverInfo": {
    "name": "codelattice",
    "version": "0.7.0",
    "cangjieSupport": true,
    "arktsSupport": true,
    "typescriptSupport": true,
    "cSupport": true,
    "cppSupport": true,
    "pythonSupport": true,
    "toolCount": 9,
    "fullToolCount": 51,
    "toolset": "ai"
  }
}
```

`codelattice-mcp.sh --version` 会显示当前 binary 的 profile：

```bash
bash scripts/codelattice-mcp.sh --version
# codelattice-mcp-wrapper 0.7.0
#   serverVersion: 0.7.0
#   cangjieSupport: True
#   arktsSupport: True
#   typescriptSupport: True
#   cSupport: True
#   cppSupport: True
#   pythonSupport: True
#   toolCount: 51
```

### 如何确认当前 binary 支持可选语言

1. `bash scripts/codelattice-mcp.sh --self-test` — 会显示 cangjieSupport / arktsSupport / typescriptSupport / cSupport / cppSupport / pythonSupport 状态
2. `bash scripts/install-mcp.sh --doctor` — 完整健康检查，包括 language support profile
3. MCP 客户端调用 `initialize` 后检查 `serverInfo.cangjieSupport` / `serverInfo.arktsSupport` / `serverInfo.typescriptSupport` / `serverInfo.cSupport` / `serverInfo.cppSupport` / `serverInfo.pythonSupport`

### 如何 rebuild

```bash
# 构建全语言 release binary
bash scripts/install-mcp.sh --build

# 仅 Rust
bash scripts/install-mcp.sh --build --rust-only

# 手动 cargo build
cargo build --release -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python --bins
```

### opencode 重启

修改 binary 后，**必须重启 opencode session** 才会重新加载 MCP server。opencode 不会在 session 内自动重启 MCP 进程。
