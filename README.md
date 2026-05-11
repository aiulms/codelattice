# CodeLattice

CodeLattice 是一个本地代码智能引擎，当前面向 Rust 与 Cangjie / 仓颉项目提供项目扫描、符号索引、调用关系解析、结构图、质量门检查和 MCP sidecar。它的目标很直接：让 AI 编程助手、代码审查和本地工程工具拿到可靠、可验证、可重复的代码上下文。

它不是一个托管服务，也不会上传你的代码。CodeLattice 运行在本机，默认以只读方式分析项目，并通过 CLI 或 MCP stdio server 对外提供能力。

当前状态：**v0.1.0 已发布，本地 CLI / MCP 工作流已可进入生产使用**。核心分析链路、GitCode Release、release tarball、checksum、release smoke、fresh clone smoke 和 MCP sidecar 已经可以用于真实 Rust / Cangjie 项目的日常开发辅助。当前最大体验缺口是 WebUI；非 WebUI 方向后续重点是多平台发行包、自动化 release CI 和外部 beta 试用。

## 适合谁

- 想让 AI agent 先理解代码结构，再回答、改动或做影响分析的开发者。
- 需要在本机生成代码图谱、符号索引、调用关系和质量门报告的团队。
- 正在维护 Rust 或 Cangjie 项目，想要一个可脚本化、可 smoke、可接 MCP client 的本地分析核心。
- 想把代码理解能力嵌入自己的工具链，但暂时不需要 WebUI 或托管平台的人。

## 核心能力

| 能力 | 说明 |
|------|------|
| 项目模型 | 识别 Rust Cargo 项目和 Cangjie cjpm 项目，建立 package、target、source ownership |
| 符号索引 | 提取函数、方法、类型、trait/interface、枚举、宏、init 等语言符号 |
| 调用解析 | 解析同模块、跨文件、import 绑定、部分关联函数和有限 receiver method |
| 图输出 | 输出 repository / package / source file / symbol / diagnostic 节点和关系边 |
| 质量门 | 检查 dangling edge、duplicate、统计一致性、stdout JSON purity、deterministic output |
| MCP sidecar | 提供 21 个 MCP 工具，支持 AI client 查询项目概览、符号上下文、调用关系、影响预览 |
| 本地安全 | 默认只读；wrapper 与 stable runtime 可隔离；配置脚本只打印模板，不写真实客户端配置 |

## Quick Start

### 1. 从 GitCode Release 安装

当前 `v0.1.0` 已发布 macOS Apple Silicon (`darwin-arm64`) 发行包：

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
curl -fsSL https://raw.gitcode.com/aiulms/codelattice/raw/master/scripts/install-release.sh \
  | bash -s -- --version v0.1.0 --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

这个 installer 会下载 GitCode Release tarball，校验 `.sha256`，安装 stable MCP wrapper，并运行 self-test。它不会修改 Codex / opencode / Claude 配置。

Linux 或其他平台当前可先走源码构建路径；多平台 release artifact 是下一步 packaging 工作。

### 2. Clone 并构建

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

# 构建 release binary，默认包含 Rust + Cangjie 支持
bash scripts/install-mcp.sh --build
```

构建后优先使用 public binary：

```bash
target/release/codelattice --version
```

兼容说明：Cargo package 仍叫 `gitnexus-rust-core-cli`，并继续构建同名兼容 binary；对外命令优先使用 `codelattice`。

### 3. 跑 fresh clone smoke

```bash
bash scripts/fresh-clone-smoke.sh --skip-tests
```

这个脚本会把当前 repo 复制到 `/tmp/codelattice-fresh-smoke-*` 模拟外部 fresh clone，不联网 clone，不触碰真实 AI client 配置。默认会验证构建、临时 stable runtime 安装、MCP wrapper self-test、tools/list、Rust fixture 和可用时的 Cangjie fixture。

需要完整测试时：

```bash
bash scripts/fresh-clone-smoke.sh
```

### 4. 安装稳定 MCP runtime

AI 客户端建议指向 promoted stable wrapper，而不是开发 checkout 里的脚本。

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

如果不传 `--install-dir`，脚本会使用默认目录 `$HOME/Desktop/CodeLattice-Tool`。

### 5. 打包 release tarball

```bash
bash scripts/check-release-metadata.sh
bash scripts/package-release.sh
bash scripts/release-smoke.sh
```

默认产物：

```text
dist/codelattice-<version>-<platform>.tar.gz
dist/codelattice-<version>-<platform>.tar.gz.sha256
```

版本规则见 `docs/release-versioning.md`，发布记录见 `CHANGELOG.md`。产品版本来自 Cargo `workspace.package.version`；MCP `serverVersion` 是 sidecar tool/profile 版本，两者分开管理。

### 6. 打印 MCP client 配置模板

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

该命令只打印 Codex / opencode / Claude 配置片段，不会修改任何真实配置。配置中的 wrapper 应指向：

```text
$CODELATTICE_TOOL_DIR/codelattice-mcp.sh
```

### 7. 分析一个 Rust fixture

```bash
target/release/codelattice analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

## CLI 用法

### 分析 Rust 项目

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json
```

严格质量门：

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json \
  --strict
```

### 分析 Cangjie / 仓颉项目

```bash
target/release/codelattice analyze \
  --root /path/to/cangjie/project \
  --language cangjie \
  --format json \
  --strict
```

### 自动检测语言

```bash
target/release/codelattice analyze \
  --root /path/to/project \
  --language auto \
  --format json
```

自动检测规则：

- 发现 `Cargo.toml`：Rust
- 发现 `cjpm.toml`：Cangjie / 仓颉
- 两者同时存在：要求显式指定 `--language`

### 质量门检查

```bash
target/release/codelattice quality \
  --root fixtures/rust/portable-smoke \
  --language rust
```

exit code：

- `0`：质量门通过
- `1`：质量门失败
- `2`：项目语言或结构不明确

### 摘要输出

```bash
target/release/codelattice summary \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

## MCP Sidecar

CodeLattice 提供 JSON-RPC over stdio 的 MCP server，可被 Codex、opencode、Claude Desktop 等支持 MCP 的客户端调用。

开发调试时可以直接用 checkout wrapper：

```bash
bash scripts/codelattice-mcp.sh --self-test
```

日常 AI client 使用建议先 promote：

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

可用 MCP 工具：

| 工具 | 用途 |
|------|------|
| `codelattice_analyze` | 分析项目，返回 graph summary + quality gates |
| `codelattice_quality` | 质量门检查，failed gates 排前面 |
| `codelattice_summary` | 紧凑概要，包含 stats + quality，无 graph |
| `codelattice_smoke` | 端到端 smoke 测试 |
| `codelattice_graph_overview` | 图规模概览，包含 node/edge/symbol counts 和 kind breakdowns |
| `codelattice_unresolved_report` | 未解析调用报告 |
| `codelattice_symbol_search` | 按名称搜索符号 |
| `codelattice_export_bridge` | 导出 bridge JSON 到 `/tmp`，供下游图谱消费 |
| `codelattice_symbol_context` | 符号上下文、调用关系摘要和源码片段 |
| `codelattice_calls_from` | 查询一个符号向外调用了什么 |
| `codelattice_calls_to` | 查询哪些符号调用了目标符号 |
| `codelattice_impact_preview` | 只读影响预览，给出风险和受影响关系 |
| `codelattice_query_graph` | 参数化本地图查询 |
| `codelattice_project_overview` | 项目级概览，适合 AI 快速建模 |
| `codelattice_repo_registry` | 只读 repo registry/status 视图 |
| `codelattice_rename_preview` | 重命名预览，只读，不写文件 |
| `codelattice_cache_status` | 查看 process-local cache 状态 |
| `codelattice_cache_clear` | 清空 process-local cache |
| `codelattice_production_assist` | 汇总质量门、未解析调用、diagnostics 和改动风险 |
| `codelattice_compare_runs` | 对比两次 bridge/run artifact 的节点和边变化 |
| `codelattice_cache_prewarm` | 预热 process-local cache，改善真实客户端首次交互体验 |

常用验证：

```bash
bash scripts/install-mcp.sh --doctor
bash scripts/codelattice-mcp.sh --self-test
bash scripts/check-release-metadata.sh
bash scripts/mcp-dogfood.sh
bash scripts/mcp-local-client-smoke.sh
bash scripts/mcp-real-client-dry-run.sh
```

## Rust 支持范围

已支持：

- Cargo package / workspace / target 识别
- source file ownership 识别
- 函数、方法、结构体、枚举、trait、impl、const、static、宏定义、enum variant 等符号提取
- `use` import 解析
- `crate::`、`self::`、`super::` 路径解析
- 同文件、同模块、跨文件 same-crate 的部分调用解析
- enum constructor / enum variant constructor 解析
- associated function 的保守解析
- receiver type 的有限方法调用启发式
- std/core/alloc 常见外部符号补全
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
- 任意第三方 crate API 深度解析

## Cangjie / 仓颉支持范围

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

启用 Cangjie feature：

```bash
cargo build --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bins
```

当前明确不做：

- 完整方法派发
- 类型推断
- trait / interface solving
- macro expansion
- 完整 cfg evaluator

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

## 安全模型

- CodeLattice 默认在本机运行，不上传项目代码。
- MCP sidecar 默认只读；`rename_preview` 只预览，不写文件。
- `export_bridge` 只写 `/tmp`。
- `install-mcp.sh --print-config` 只打印配置模板，不修改 Codex / opencode / Claude 配置。
- `fresh-clone-smoke.sh` 默认使用 `/tmp` 临时目录，结束后清理。

## 项目状态与路线图

当前可以依赖的部分：

- Rust / Cangjie CLI 分析
- Rust / Cangjie 质量门
- MCP sidecar 21 工具
- stable runtime promote
- release tarball packaging + release smoke
- fresh clone smoke
- 本地 AI client 接入模板

正在补齐的部分：

- WebUI
- 正式发布流程与版本资产发布
- Cargo package 名称的 CodeLattice 迁移
- 更多平台和外部环境验证
- 更丰富的 Rust method dispatch 与 Cangjie diagnostics 能力

长期方向：

- 成为一个可嵌入、可验证、可扩展的多语言代码智能核心
- 为本地代码理解、影响分析、重构辅助和 AI agent 工作流提供基础设施

## 开发与验证

构建：

```bash
./scripts/build.sh
```

快速 smoke：

```bash
./scripts/smoke.sh --quick
```

完整本地验证：

```bash
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
bash scripts/install-mcp.sh --doctor
bash scripts/codelattice-mcp.sh --self-test
bash scripts/package-release.sh
bash scripts/release-smoke.sh
bash scripts/fresh-clone-smoke.sh --skip-tests
```

更完整的 MCP 验证：

```bash
bash scripts/mcp-dogfood.sh
bash scripts/mcp-real-client-dry-run.sh
bash scripts/mcp-local-client-smoke.sh
```

## 项目结构

```text
codelattice/
  Cargo.toml
  crates/
    project-model/       Rust 项目模型、符号、import、calls、graph 输出
    cangjie/             Cangjie 项目模型、符号、diagnostics、graph 输出
    cli/                 命令行入口、统一输出、MCP server、语言检测
  fixtures/
    call-resolution/     Rust 调用解析 fixture
    import-use/          Rust import fixture
    item-extraction/     Rust 符号提取 fixture
    rust/                Rust graph contract fixture
    cangjie/             Cangjie fixture
  docs/
    architecture/        架构和输出格式说明
    decisions/           设计决策
    fixtures/            fixture 索引
    plans/               preflight / execution / closure 文档
  scripts/
    build.sh
    smoke.sh
    codelattice-mcp.sh
    install-mcp.sh
    promote-to-local-tool.sh
    package-release.sh
    release-smoke.sh
    fresh-clone-smoke.sh
```

## License

MIT License. See [LICENSE](LICENSE).
