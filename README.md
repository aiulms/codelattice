# CodeLattice

CodeLattice 是一个面向 Rust 与 Cangjie / 仓颉项目的本地代码图谱分析核心，提供符号提取、调用关系解析、结构图生成、质量门检查、Bridge JSON 导出与 MCP AI sidecar 能力。它的目标是给 AI 编程工具和代码审查工作流提供可验证、可回归、可清理的本地事实上下文。

> 旧工作名：`gitnexus-rust-core`。当前 alpha 阶段的部分 crate、binary、兼容 flag 仍保留旧名，后续会按兼容计划逐步治理。

> 当前状态：**Alpha Production Trial ACTIVE / PASSING**。Rust 与 Cangjie 核心链路已通过多轮真实项目 trial，MCP v0.5 已进入 daily-use candidate。项目仍是显式 opt-in，不是 Beta / GA，也不会默认替代 GitNexus-RC 或任何既有生产链路。

当前重点支持两类语言：

- Rust：Cargo 项目扫描、符号提取、import 解析、调用关系、图输出和质量门。
- Cangjie / 仓颉：cjpm 项目扫描、符号提取、import/reference/call 解析、diagnostics 接入、图输出和质量门。

这个仓库已经完成从“语言分析核心”到“可给 AI 日常调用的本地 sidecar”的主要闭环：Rust 自分析和 Cangjie cjgui 真实项目 trial 多轮通过，stdout JSON 纯净，0 dangling、0 duplicate，Tool bridge ingestion 成功，外部 AI 独立 retry 已 PASS 并计入 Beta evidence。当前重点是继续积累 Beta 所需的日历跨度和剩余 trial，而不是短期扩展 Web UI、默认替换或多语言大覆盖。

## Quick Start（外部 fresh clone）

CodeLattice 是 Rust/Cangjie 本地代码智能核心，不是 GitNexus-RC 的替代品。GitNexus-RC 仍是治理来源、跨仓 runtime/adapter/schema 和 Tool/WebUI 主线；CodeLattice 提供本地 Rust/Cangjie 分析、质量门和 MCP sidecar。

当前状态是 **Alpha / daily-use candidate**：本机 production trial 与真实客户端 smoke 已 PASS，但还不是 Beta / GA。支持语言主线是 Rust + Cangjie，其他语言是未来扩展。

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

# 构建 release binary（默认 Rust + Cangjie）
bash scripts/install-mcp.sh --build

# 可选：验证 fresh clone 外部复用路径
bash scripts/fresh-clone-smoke.sh --skip-tests
```

把 MCP runtime 推广到稳定目录（AI 客户端应指向这里，而不是开发 checkout）：

```bash
export CODELATTICE_TOOL_DIR="$HOME/Desktop/CodeLattice-Tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

打印 Codex / opencode / Claude 配置片段：

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

该命令只打印模板，不修改任何真实客户端配置。配置中的 wrapper 应是：

```text
$CODELATTICE_TOOL_DIR/codelattice-mcp.sh
```

分析 Rust fixture：

```bash
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

> `gitnexus-rust-core-cli` 是旧工作名留下的 Cargo package / binary 兼容名，不影响 CodeLattice 当前项目身份。Cargo bin rename 属于后续兼容迁移计划。

## 现在做到哪一步

| 方向 | 当前状态 |
|------|----------|
| 生产试用状态 | Alpha Production Trial ACTIVE / PASSING；Run #001/#002/#004 PASS，Run #003 失败记录保留且不计入；External AI independent run 已 PASS |
| Beta readiness | 当前 3/5 beta-countable PASS logs；Tool ingestion / stdout purity / external AI criterion 均 PASS；剩余 gap 是日历跨度 + 2 次 PASS trial |
| 统一 CLI | 已有 `analyze`、`quality`、`summary` 三个入口，支持 `--language auto` 和 `analyze --strict` |
| Rust 项目模型 | 支持 Cargo manifest、workspace、target、source ownership |
| Rust 符号与调用 | 支持函数、方法、结构体、枚举、trait、impl、const、static、宏定义、enum variant 等符号 |
| Rust 调用解析 | 当前 self-smoke 基线为 2370/3609，解析率 65.7% |
| Rust 图质量 | Run #004 bridge：1702 nodes / 2635 edges，0 duplicate，0 dangling，输出 deterministic；Tool ingestion：5102 nodes / 7545 edges |
| Rust 回归测试 | graph contract 58/58，覆盖 8 个图合同 fixture；call comparison 24/24 fixture |
| 仓颉项目模型 | 支持 `cjpm.toml`、workspace members、source files、path dependencies、外部依赖信息 |
| 仓颉符号与关系 | 支持 Function、Class、Struct、Enum、Interface、TypeAlias、Macro、Init，支持 import/reference/call graph |
| 仓颉质量门 | Run #004 bridge：903 nodes / 3252 edges，0 duplicate，0 dangling；Tool ingestion：7219 nodes / 14314 edges；graph_contract 24/24，cangjie_inspect 18/18 |
| 试用脚本 | 提供 `scripts/build.sh`、`scripts/smoke.sh`、`scripts/alpha-trial-smoke.sh`、`scripts/mcp-dogfood.sh`、`scripts/codelattice-mcp.sh`、`scripts/install-mcp.sh`、`scripts/promote-to-local-tool.sh`、`scripts/mcp-local-client-smoke.sh`、`scripts/mcp-cache-smoke.sh` |
| MCP stdio | v0.7+ daily-use candidate：21 个工具，JSON-RPC over stdio，process-local cache、mtime invalidation、LRU、source snippet、production assist、compare runs、cache prewarm |
| MCP 验证 | MCP tests 52/52、dogfood 22/22、local client smoke 9/9、cache smoke 4/4、real client dry-run 10/10、doctor 7/7 |
| MCP Sidecar | 开发调试用 `scripts/codelattice-mcp.sh`；AI IDE 日常使用应指向 `$CODELATTICE_TOOL_DIR/codelattice-mcp.sh` 稳定运行目录；详见 `docs/architecture/mcp-local-client-setup.md` |
| 本机默认替换 | 已完成 preflight；推荐 language-aware wrapper，但尚未启用，仍不切默认工具 |

## 生产试用边界

Alpha Production Trial 的含义是：可以在真实项目中作为 AI 编程助手的前置本地分析工具使用，但仍需显式 opt-in、按 runbook 运行、记录 trial log，并保留回滚路径。

已经完成：

- Rust 真实 workspace 项目多轮分析通过。
- Cangjie 真实项目多轮分析通过。
- 输出 JSON stdout 纯净，不需要额外清洗。
- 质量门覆盖 duplicate、dangling、deterministic、stats consistency 等关键问题。
- 下游图谱导入验证通过。
- 已有 runbook、failure playbook、periodic trial log、beta readiness criteria 和 evidence board。
- 外部 AI 独立 retry（Run #004）通过并计入 Beta criteria。
- MCP v0.7+ 已具备本地 AI client daily-use candidate 能力，并通过 `CodeLattice-Tool` 稳定运行目录与开发 checkout 隔离。

尚未承诺：

- 默认生产引擎。
- Beta / GA 稳定性。
- GitNexus-RC 替代版。
- Web UI / 默认 MCP 切换。
- 多语言完整替代版。
- 无人值守长期运行。

## 能分析出什么

### Rust

已支持：

- Cargo package / workspace / target 识别
- source file ownership 识别
- Rust 符号提取
- `use` import 解析
- `crate::`、`self::`、`super::` 路径解析
- 同文件、同模块、跨文件 same-crate 的部分调用解析
- enum constructor / enum variant constructor 解析
- associated function 的保守解析
- receiver type 的有限方法调用启发式
- std/core/alloc 常见外部符号补全
- graph 节点和边输出
- graph endpoint integrity 质量门

当前 Rust 调用解析支持的代表形式：

| 调用形式 | 示例 | 置信度 |
|----------|------|--------|
| 同模块函数 | `helper()` | 0.90 |
| import 绑定 | `use crate::math::add; add()` | 0.85 |
| `crate::` 路径 | `crate::math::add()` | 0.90 |
| `self::` 路径 | `self::inner_helper()` | 0.80 |
| `super::` 路径 | `super::parent_fn()` | 0.80 |
| associated function | `Config::new()` | 0.75 |
| enum constructor | `Some(42)`、`Ok(value)`、`Err(error)` | 0.80 |
| enum variant constructor | `Event::Click(x)` | 0.80 |
| 跨文件 same-crate 函数 | `split_last_segment()` | 0.80 |
| wildcard import 消歧 | `helper_func()`，通过 `use calculations::*` 引入 | 0.80 |
| 有限 receiver method | `v.push(1)` where `let v: Vec<i32>` | 0.65 |

当前明确不做：

- 完整类型推断
- trait solving
- proc-macro / build.rs 执行
- macro expansion
- 完整 cfg evaluator
- 任意第三方 crate API 解析

### Cangjie / 仓颉

已支持：

- `cjpm.toml` package / workspace 扫描
- source file 收集
- Function / Class / Struct / Enum / Interface / TypeAlias / Macro / Init 符号提取
- named import / alias import / wildcard import / path dependency 解析
- same-file 和 cross-file reference extraction
- function call reference extraction
- `cjc` / `cjlint` diagnostics runner 接入
- graph 输出
- `cangjie inspect` / `cangjie graph`
- `--strict` 质量门

仓颉能力需要开启 feature：

```bash
cargo build --features tree-sitter-cangjie -p gitnexus-rust-core-cli
```

当前明确不做：

- 完整方法派发
- 类型推断
- trait / interface solving
- macro expansion
- 完整 cfg evaluator

## 快速开始

### 构建

```bash
./scripts/build.sh
```

默认构建 release 版本，并包含仓颉支持。

可选参数：

```bash
./scripts/build.sh --debug
./scripts/build.sh --no-cangjie
```

构建产物：

```bash
target/release/gitnexus-rust-core-cli
```

> `gitnexus-rust-core-cli` 是 alpha 阶段保留的兼容 binary 名，不代表当前项目身份。

### 快速 smoke

```bash
./scripts/smoke.sh --quick
```

完整验证：

```bash
./scripts/smoke.sh
```

### Alpha 端到端 smoke

```bash
# Rust + Cangjie 端到端验证
./scripts/alpha-trial-smoke.sh

# 仅验证 Rust
./scripts/alpha-trial-smoke.sh --rust-only

# 仅验证 Cangjie
./scripts/alpha-trial-smoke.sh --cangjie-only
```

验证项：结构完整性、端点完整性、统计一致性、stdout JSON purity、下游图谱导入、输出确定性。

## CLI 用法

### 分析 Rust 项目

```bash
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

严格模式：

```bash
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json \
  --strict
```

### 分析仓颉项目

```bash
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- analyze \
  --root fixtures/cangjie/portable-smoke \
  --language cangjie \
  --format json \
  --strict
```

### 自动检测语言

```bash
cargo run -p gitnexus-rust-core-cli -- analyze \
  --root /path/to/project \
  --language auto \
  --format json
```

自动检测规则：

- 发现 `Cargo.toml`：Rust
- 发现 `cjpm.toml`：仓颉
- 两者同时存在：要求显式指定 `--language`

### 质量门检查

```bash
cargo run -p gitnexus-rust-core-cli -- quality \
  --root fixtures/rust/portable-smoke \
  --language rust
```

exit code：

- `0`：质量门通过
- `1`：质量门失败
- `2`：项目语言或结构不明确

### 摘要输出

```bash
cargo run -p gitnexus-rust-core-cli -- summary \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

### MCP stdio server

CodeLattice 提供 MCP JSON-RPC stdio server，允许 AI agent 通过标准协议调用分析能力。

```bash
# 直接启动 MCP server
cargo run -p gitnexus-rust-core-cli -- mcp

# 开发 checkout wrapper（调试 CodeLattice 本身时使用）
bash scripts/codelattice-mcp.sh --self-test

# 推广到 AI IDE 使用的稳定运行目录
export CODELATTICE_TOOL_DIR="$HOME/Desktop/CodeLattice-Tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

可用工具（21 个）：

| 工具 | 用途 |
|------|------|
| `codelattice_analyze` | 分析项目，返回 graph summary + quality gates（默认 compact） |
| `codelattice_quality` | 质量门检查（failed gates 排前面） |
| `codelattice_summary` | 紧凑概要（stats + quality，无 graph） |
| `codelattice_smoke` | 端到端 smoke 测试 |
| `codelattice_graph_overview` | 图规模概览（node/edge/symbol counts + kind breakdowns） |
| `codelattice_unresolved_report` | 未解析调用报告（Rust only; Cangjie returns supported=false） |
| `codelattice_symbol_search` | 按名称搜索符号（case-insensitive substring） |
| `codelattice_export_bridge` | 导出 bridge JSON 到 /tmp（GitNexus-RC 兼容格式） |
| `codelattice_symbol_context` | 符号上下文、调用关系摘要和源码片段 |
| `codelattice_calls_from` | 查询一个符号向外调用了什么 |
| `codelattice_calls_to` | 查询哪些符号调用了目标符号 |
| `codelattice_impact_preview` | 只读影响预览，给出风险和受影响关系 |
| `codelattice_query_graph` | 参数化本地图查询（不是任意 Cypher） |
| `codelattice_project_overview` | 项目级概览，适合 AI 快速建模 |
| `codelattice_repo_registry` | 只读 repo registry/status 视图 |
| `codelattice_rename_preview` | 重命名预览，只读，不写文件 |
| `codelattice_cache_status` | 查看 process-local cache 状态 |
| `codelattice_cache_clear` | 清空 process-local cache |
| `codelattice_production_assist` | 基于当前分析结果给出生产辅助建议、风险和下一步 |
| `codelattice_compare_runs` | 对比两次 bridge/run artifact 的节点和边变化 |
| `codelattice_cache_prewarm` | 预热 process-local cache，改善真实客户端首次交互体验 |

Safety: path deny list 阻止 live repo 访问；export_bridge 仅写 `/tmp`；rename_preview 不落盘；MCP sidecar 默认 read-only。详见 `docs/architecture/mcp-v0-contract.md`。

常用验证：

```bash
bash scripts/codelattice-mcp.sh --self-test
bash scripts/promote-to-local-tool.sh
bash scripts/mcp-dogfood.sh
bash scripts/mcp-local-client-smoke.sh
bash scripts/mcp-cache-smoke.sh
bash scripts/install-mcp.sh --doctor
```

最新 daily-use candidate 记录：dogfood 22/22、local client smoke 9/9、cache smoke 4/4、real client dry-run 10/10、doctor 7/7。

## 输出内容

`analyze --format json` 会输出统一分析结果，主要包含：

- 项目摘要
- 质量门结果
- 语言信息
- graph 节点和边
- diagnostics
- stats

图数据包含的常见节点：

- Repository
- Package
- Target
- SourceFile
- Symbol
- Diagnostic

图数据包含的常见关系：

- CONTAINS_PACKAGE
- HAS_TARGET
- OWNS_SOURCE
- DEFINES
- CALLS
- ACCESSES
- DESIGNATION
- HAS_PARENT
- ANNOTATES

## 验证命令

```bash
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
cargo test --test project_model_graph_contract
cargo test --test productization_commands
cargo test --features tree-sitter-cangjie --test cangjie_inspect -- --nocapture
cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture
cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --nocapture
```

## 项目结构

```text
codelattice/
  Cargo.toml
  crates/
    project-model/       Rust 项目模型、符号、import、calls、graph 输出
    cangjie/             仓颉项目模型、符号、diagnostics、graph 输出
    cli/                 命令行入口、统一输出、语言检测
  fixtures/
    call-resolution/     Rust 调用解析 fixture
    import-use/          Rust import fixture
    item-extraction/     Rust 符号提取 fixture
    rust/                Rust graph contract fixture
    cangjie/             仓颉 fixture
  docs/
    architecture/        架构和输出格式说明
    decisions/           设计决策
    fixtures/            fixture 索引
    plans/               preflight / execution / closure 文档
  scripts/
    build.sh             本地构建脚本
    smoke.sh             本地 smoke 验证脚本
    alpha-trial-smoke.sh Alpha trial 端到端验证
    codelattice-mcp.sh   MCP sidecar 开发 wrapper
    install-mcp.sh       MCP client 配置片段与 doctor
    promote-to-local-tool.sh  推广到稳定 AI IDE runtime
```

## 路线图

短期目标：

- 完成 Run #005 和后续 1 次 beta-countable PASS trial
- 等待满足 Alpha -> Beta 所需的日历跨度
- 继续维护 MCP daily-use candidate，保持 dogfood / local client / cache / doctor smoke
- 对外复用前完成 External Reuse Pack：路径参数化、fresh clone smoke、release/install 文档
- 保持 Rust / Cangjie quality gates、stdout purity、bridge ingestion 和 registry cleanup 稳定
- 本机默认替换只推进 opt-in language-aware wrapper，必须先经过用户批准

中期目标：

- 提升 Rust 调用解析质量
- 建立更完整的 confidence / reason 矩阵
- 将仓颉 SDK、LSP、diagnostics 能力更系统地接入
- 继续打磨 MCP / IDE / 下游工具消费层
- 达成 Beta opt-in readiness 后再评估更广泛的默认入口策略

长期目标：

- 成为一个可嵌入、可验证、可扩展的多语言代码智能核心
- 为本地代码理解、影响分析、重构辅助、AI agent 工作流提供基础设施

## 当前边界

这个项目优先追求“结构化、可验证、可回归”的代码智能能力，而不是追求一次性覆盖所有语义。

当前不会做：

- 运行用户项目的 `build.rs` / proc-macro
- 宏展开
- 完整类型推断
- 完整 trait solving
- 任意第三方 crate API 深度解析
- Web UI / 默认生产服务
- 默认替代 GitNexus-RC 或其他生产工具
- 自动修改用户 MCP / shell / AI client 配置
- 商业化分发承诺

## 许可证

本项目采用 MIT License，详见 [LICENSE](LICENSE)。
