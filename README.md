# CodeLattice

> 中文 README 是本项目的权威介绍与维护基准；英文说明仅作为外部 beta 用户的参考入口。

CodeLattice 是一个用 Rust 编写的本地代码图谱分析核心，面向 AI 编程工具、代码审查和工程质量检查提供可验证的代码理解能力。它目前重点支持 Rust 与 Cangjie / 仓颉项目，并逐步覆盖 ArkTS / HarmonyOS 与 TypeScript。

它做的事情很直接：在本地读取代码，抽取符号，解析调用关系，生成结构化图谱，检查图谱质量，然后通过 CLI 和 MCP sidecar 把这些结果交给 AI 编程助手或工程工具使用。整个过程默认只读，不上传代码，不依赖云端索引。

**当前状态：外部 Beta（`v0.13.0-beta.2`）**。本地生产试用已通过，但还不是 GA 版本。完整变更见 [CHANGELOG](CHANGELOG.md)，验证矩阵见 [Smoke Matrix](docs/release/smoke-matrix.md)。

英文参考页：[docs/README.en.md](docs/README.en.md)

## 项目介绍

CodeLattice 的目标不是再做一个“代码搜索工具”，而是提供一个可嵌入、可验证、可自动化的本地代码智能底座。它把仓库中的源码转成 AI 和工程工具都能稳定消费的结构化上下文，包括：

- 项目、包、目标、源码文件和符号之间的所有权关系
- 函数、方法、类型、接口、枚举、宏、初始化函数等语言符号
- 同文件、跨文件、导入绑定、有限关联函数和有限方法调用关系
- 可量化的图谱质量检查、悬空边检查、重复检查和统计一致性检查
- 面向 AI 代码审查的影响范围、风险理由、低置信度边和 review focus

换句话说，CodeLattice 负责把“代码长什么样、谁调用谁、改动可能影响哪里”这类上下文，从临时 grep 和人工猜测，变成稳定、可复跑、可审计的本地数据。

## 为什么适合研究大型遗留代码

很多真实项目不是从干净架构开始的：文件很大、调用链很长、模块边界模糊、文档和实现不一致，俗称“屎山代码”。CodeLattice 的价值不是假装一键重构这些项目，而是先把它们变成 AI 和人都能看懂的工程地图。

- **先看清再下手**：通过 project overview、symbol search、call graph 和质量 gate，先定位大文件、高扇出符号、低置信度调用和潜在风险区域。
- **改前知道影响面**：`impact_preview` 会告诉 AI 一个函数被哪些调用方依赖、影响跨了哪些文件、为什么风险高，以及哪些边需要人工复核。
- **改后自动复盘**：`changed_symbols` 能从 git diff 自动识别改了哪些符号，`production_assist` 会生成 review checklist，并提示可能需要同步更新的文档。
- **适合大项目重复分析**：核心分析链路用 Rust 实现，默认静态读取源码，不执行用户项目脚本；memory + persistent 两层缓存减少真实 MCP 使用中的重复全量分析。
- **对 AI 更友好**：相比让 AI 在一堆文件里临时搜索，CodeLattice 提供的是带 `id / file / line / confidence / reason` 的结构化上下文，更适合安全改动和代码审查。

所以 CodeLattice 特别适合接手陌生大仓、研究高耦合模块、拆解历史包袱重的代码、评估重构风险，以及让 AI 在复杂项目里“先问图谱，再改代码”。

## 为什么用 Rust

CodeLattice 选择 Rust 不是包装层面的卖点，而是产品能力的一部分。

- **本地执行更稳**：Rust 适合构建单文件二进制和轻量 sidecar，便于放进 Codex、opencode、Claude Desktop 等本地 AI 工作流里长期运行。
- **性能和资源占用可控**：符号抽取、调用分析、图谱构建和缓存复用都需要频繁扫描源码；Rust 的性能和内存模型适合做这类高频本地分析。
- **安全边界清晰**：默认只读分析，不执行用户项目脚本；Rust 的内存安全和显式错误处理让本地工具更容易保持可预期行为。
- **输出更容易验证**：CodeLattice 重视确定性输出、质量 gate、confidence / reason 和 smoke fixture；Rust 让这些底层约束更容易固化在测试与发布流程里。
- **跨平台发布路径明确**：当前已发布 macOS Apple Silicon 包，Linux / openEuler 走源码构建验证；后续扩展到更多平台时，Rust 工具链路径相对清晰。

## 相比类似工具的优势

| 对比对象 | CodeLattice 的差异 |
|----------|--------------------|
| 云端代码智能服务 | 本地运行，默认不上传代码，适合私有仓库和受控环境 |
| 纯 IDE / LSP 能力 | 输出面向自动化消费，不绑定某个编辑器，可通过 CLI / MCP 接入多种工具 |
| grep / ripgrep / ctags | 不只做文本命中，还生成项目模型、符号图、调用边和质量报告 |
| 编译器或完整静态分析器 | 不追求完整类型推断和 trait solving，而是提供工程上可解释、可标注置信度的快速上下文 |
| 通用多语言扫描器 | 对 Rust 与 Cangjie 有更深的项目模型、fixture、质量 gate 和调用关系策略 |
| AI 插件内置索引 | CodeLattice 是独立本地核心，可复用、可测试、可离线、可审计 |

这也是项目名字里 “Lattice” 的含义：把分散的符号、文件、调用、质量信号和变更影响织成一个可查询的局部结构，让 AI 不只是“读一段代码”，而是能沿着工程关系理解项目。

## 适合谁使用

- 希望 AI 在改代码前先理解项目结构、调用关系和影响范围的开发者
- 正在接手陌生大仓、遗留系统或高耦合“屎山代码”，需要先看清风险再动手的团队
- 需要在本地生成代码图谱、符号索引、调用关系和质量报告的团队
- 维护 Rust 或 Cangjie / 仓颉项目，并希望有可脚本化、可 smoke、可接 MCP 的分析核心的工程团队
- 希望把代码理解能力嵌入自有工具链，而不想依赖 Web UI 或托管平台的用户

## 核心能力

| 能力 | 说明 |
|------|------|
| 项目模型 | 识别 Rust Cargo、Cangjie cjpm、ArkTS HarmonyOS 项目，建立 package / target / source ownership |
| 符号抽取 | 抽取函数、方法、类型、trait / interface、enum、macro、init 等符号；ArkTS 额外抽取 `@Component`、`@State`、`build()` |
| 调用解析 | 支持同模块、跨文件、导入绑定、部分关联函数、有限 receiver method 等调用解析 |
| 图谱输出 | 输出 repository / package / source file / symbol / diagnostic 节点和关系边 |
| 质量 gate | 检查悬空边、重复项、统计一致性、stdout JSON 纯净度和输出确定性 |
| MCP sidecar | 提供 22 个 MCP 工具，支持项目概览、符号上下文、调用查询、影响预览、变更符号检测和文档关联 |
| 持久化缓存 | memory + disk 两层缓存，支持 fingerprint invalidation，减少真实 AI client 的重复分析成本 |
| 本地安全 | 默认只读；配置脚本默认只打印模板，不写入 Codex / opencode / Claude 等真实配置 |

## 快速开始

### 1. 克隆并构建

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

# 构建默认全语言 adapter 支持的 release binary
bash scripts/install-mcp.sh --build
```

构建后使用公开命令：

```bash
target/release/codelattice --version
```

兼容说明：Cargo package 仍保留历史名称 `gitnexus-rust-core-cli`，并继续生成兼容二进制；新的外部命令名推荐使用 `codelattice`。

### 2. 运行 fresh clone smoke

```bash
bash scripts/fresh-clone-smoke.sh --skip-tests
```

该脚本会把当前仓库复制到 `/tmp/codelattice-fresh-smoke-*`，模拟外部 fresh clone，不触碰真实 AI client 配置。默认验证构建、临时 stable runtime 安装、MCP wrapper self-test、tools/list、Rust fixture 和可用时的 Cangjie fixture。

完整测试：

```bash
bash scripts/fresh-clone-smoke.sh
```

### 3. 安装稳定 MCP runtime

AI client 应优先指向 promoted stable wrapper，而不是开发 checkout 里的脚本。

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

如果不传 `--install-dir`，脚本默认使用 `$HOME/Desktop/CodeLattice-Tool`。

### 4. 分析 Rust fixture

```bash
target/release/codelattice analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

### 5. 从 Release 包安装

当前 `v0.13.0-beta.2` 已发布 macOS Apple Silicon（`darwin-arm64`）包：

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.13.0-beta.2 \
  --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

安装器会下载 GitCode Release tarball，校验 `.sha256`，安装稳定 MCP wrapper，并运行 self-test。它不会修改 Codex / opencode / Claude 配置。更多细节见 [安装指南](docs/release-install.md) 和 [升级指南](docs/release/upgrade.md)。

Linux 和其他平台当前走源码构建路径；多平台 release artifact 是下一阶段包装目标。openEuler / Linux 依赖与 smoke 命令见 [Linux / openEuler 源码构建指南](docs/platforms/linux-openeuler.md)。

### 6. 打印 MCP client 配置模板

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

该命令只打印 Codex / opencode / Claude 配置片段，不修改真实配置。配置里的 wrapper 应指向：

```text
$CODELATTICE_TOOL_DIR/codelattice-mcp.sh
```

## 支持语言

| 语言 | 状态 | Feature Flag |
|------|------|--------------|
| Rust | **Stable** | `tree-sitter-extraction`，默认启用 |
| Cangjie / 仓颉 | **Stable** | `tree-sitter-cangjie` |
| ArkTS / HarmonyOS | **Production Trial** | `tree-sitter-arkts` |
| TypeScript | **Phase A** | `tree-sitter-typescript` |

## CLI 使用

### 分析 Rust 项目

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json
```

启用严格质量 gate：

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

### 分析 ArkTS / HarmonyOS 项目

```bash
target/release/codelattice analyze \
  --root /path/to/arkts/project \
  --language arkts \
  --format json
```

导出 GitNexus-RC bridge 格式：

```bash
target/release/codelattice analyze \
  --root /path/to/arkts/project \
  --language arkts \
  --format gitnexus-rc
```

### 自动识别语言

```bash
target/release/codelattice analyze \
  --root /path/to/project \
  --language auto \
  --format json
```

自动识别规则：

- 找到 `Cargo.toml`：Rust
- 找到 `cjpm.toml`：Cangjie / 仓颉
- 找到 `oh-package.json5`：ArkTS
- 同时检测到多个语言：需要显式传入 `--language`

### 质量检查

```bash
target/release/codelattice quality \
  --root fixtures/rust/portable-smoke \
  --language rust
```

退出码：

- `0`：质量 gate 通过
- `1`：质量 gate 失败
- `2`：项目语言或结构不明确

## MCP Sidecar

CodeLattice 提供基于 stdio JSON-RPC 的 MCP server，可被 Codex、opencode、Claude Desktop 等 MCP client 调用。

开发调试可以直接使用 checkout wrapper：

```bash
bash scripts/codelattice-mcp.sh --self-test
```

日常 AI client 使用建议先 promote：

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

常用 MCP 工具：

| 工具 | 说明 |
|------|------|
| `codelattice_project_overview` | 项目级概览，适合 AI 快速建模 |
| `codelattice_symbol_context` | 符号上下文、调用关系摘要和源码片段 |
| `codelattice_calls_from` | 查询某个符号向外调用了什么 |
| `codelattice_calls_to` | 查询哪些符号调用目标符号 |
| `codelattice_impact_preview` | 只读影响预览，返回风险级别、风险理由、影响指标、置信度摘要和 review focus |
| `codelattice_changed_symbols` | 从 git diff 自动识别变更涉及的符号 |
| `codelattice_production_assist` | 一站式摘要：quality gates、unresolved calls、diagnostics、change risk、review checklist |
| `codelattice_cache_status` | 查看 memory + persistent 两层缓存状态 |
| `codelattice_cache_clear` | 清理 memory / persistent / both 缓存层 |

AI 编程助手推荐使用这条链路完成“改代码 -> 看影响 -> 审查 -> 提交”的闭环：

1. `codelattice_project_overview`：快速理解项目规模
2. `codelattice_changed_symbols`：识别当前 git diff 影响的符号
3. `codelattice_impact_preview`：查看影响范围、风险理由、置信度和 review focus
4. `codelattice_production_assist`：汇总质量 gate、未解析调用、变更影响和审查清单

## Rust 支持范围

已支持：

- Cargo package / workspace / target 识别
- Source file ownership 识别
- 函数、方法、struct、enum、trait、impl、const、static、macro definition、enum variant 抽取
- `use` import resolution
- `crate::`、`self::`、`super::` path resolution
- 部分 same-file、same-module、cross-file same-crate call resolution
- enum constructor / enum variant constructor resolution
- 保守 associated function resolution
- 有限 receiver type method call heuristic
- 常见 std / core / alloc external symbol completion
- Graph endpoint integrity quality gate

代表性 Rust 调用解析形式：

| 调用形式 | 示例 | 置信度 |
|----------|------|--------|
| 同模块函数 | `helper()` | 0.90 |
| 导入绑定 | `use crate::math::add; add()` | 0.85 |
| `crate::` 路径 | `crate::math::add()` | 0.90 |
| `self::` 路径 | `self::inner_helper()` | 0.80 |
| `super::` 路径 | `super::parent_fn()` | 0.80 |
| 关联函数 | `Config::new()` | 0.75 |
| 枚举构造 | `Some(42)`、`Ok(value)`、`Err(error)` | 0.80 |
| 枚举变体构造 | `Event::Click(x)` | 0.80 |
| 跨文件同 crate 函数 | `split_last_segment()` | 0.80 |
| wildcard import 消歧 | `helper_func()` via `use calculations::*` | 0.80 |
| 有限 receiver method | `v.push(1)` where `let v: Vec<i32>` | 0.65 |

明确不支持：

- 完整类型推断
- trait solving
- proc-macro / build.rs 执行
- 宏展开
- 完整 cfg evaluator
- 任意第三方 crate API 深度解析

## Cangjie / 仓颉支持范围

已支持：

- `cjpm.toml` package / workspace 扫描
- source file collection
- Function / Class / Struct / Enum / Interface / TypeAlias / Macro / Init 符号抽取
- named import / alias import / wildcard import / path dependency resolution
- same-file 和 cross-file reference extraction
- function call reference extraction
- `cjc` / `cjlint` diagnostics runner integration
- graph output
- `cangjie inspect` / `cangjie graph`
- `--strict` quality gate

启用 Cangjie feature：

```bash
cargo build --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bins
```

明确不支持：

- 完整 method dispatch
- 类型推断
- trait / interface solving
- 宏展开
- 完整 cfg evaluator

## 缓存与性能

CodeLattice 提供两层分析缓存，用于加速重复 MCP 调用：

1. **Memory Layer**：默认启用，进程内 LRU cache，最多 16 个 entry。同一进程内重复调用可直接命中。
2. **Persistent Layer**：可选跨进程磁盘缓存，通过 `CODELATTICE_CACHE_DIR` 启用。

缓存查找顺序：memory -> persistent -> re-analysis。

| 环境变量 | 说明 |
|----------|------|
| `CODELATTICE_CACHE_DIR` | 持久化缓存目录；设置后启用 persistent cache |
| `CODELATTICE_CACHE` | 设置为 `off` 可完全关闭 memory 和 persistent cache |

缓存会在源码文件、构建配置、CodeLattice 版本或缓存文件损坏时自动失效，并返回结构化 `staleReasons`，方便 AI client 理解缓存行为。

## 输出内容

`analyze --format json` 输出统一分析结果，主要包含：

- project summary
- quality gate results
- language information
- graph nodes and edges
- diagnostics
- stats

常见图节点：

- Repository
- Package
- Target
- SourceFile
- Symbol
- Diagnostic

常见关系：

- CONTAINS_PACKAGE
- HAS_TARGET
- OWNS_SOURCE
- DEFINES
- CALLS
- ACCESSES
- DESIGNATION
- HAS_PARENT
- ANNOTATES

## 已知边界

- CodeLattice 不是编译器、IDE 或语言服务器；不做完整类型推断、trait solving 或宏展开
- 调用边是带 confidence / reason 的启发式分析结果，不是编译器证明
- **TypeScript**：暂无 path alias resolution、monorepo / workspace 支持和 TSX framework hints
- **ArkTS**：`struct` 关键字由 tree-sitter-typescript 解析为 ERROR node，当前通过 pattern matching 恢复；暂不支持 `@Builder` / `@Extend`
- 不执行用户项目脚本
- 暂无 per-symbol incremental recompute，目前仍以项目级重新分析为主

## 安全模型

- 默认本地运行，不上传项目代码
- MCP sidecar 默认只读；`rename_preview` 只预览，不写文件
- `export_bridge` 只写入 `/tmp`
- `install-mcp.sh --print-config` 只打印配置模板，不修改 Codex / opencode / Claude 配置
- `fresh-clone-smoke.sh` 默认使用 `/tmp` 临时目录，并在结束后清理

## 项目状态与路线图

**外部 Beta（`v0.13.0-beta.2`）**：本地生产试用已通过，但不是 GA。

当前相对可靠：

- Rust / Cangjie CLI 分析（Stable）
- ArkTS CLI 分析（Production Trial）
- TypeScript CLI 分析（Phase A）
- MCP sidecar 22 个工具
- 两层持久化缓存
- stable runtime promote
- release tarball packaging + release smoke
- fresh clone smoke
- 本地 AI client 集成模板

正在改进：

- Linux、Windows 等多平台 release 包
- Linux / openEuler native smoke certification
- 自动化 release CI
- TypeScript path alias / monorepo 支持
- TSX framework hints
- 更深的 per-symbol incremental recompute

长期方向：

- 成为可嵌入、可验证、可扩展的多语言代码智能核心
- 为本地代码理解、影响分析、重构辅助和 AI agent 工作流提供基础设施

## 文档

- [CHANGELOG](CHANGELOG.md)：版本变更记录
- [MCP Contract](docs/architecture/mcp-v0-contract.md)：MCP 工具输入输出契约
- [Unified Output Contract](docs/architecture/unified-output-contract.md)：CLI 输出格式
- [Release Versioning](docs/release-versioning.md)：版本规则
- [Install Guide](docs/release-install.md)：tarball 安装说明
- [Linux / openEuler Source Build](docs/platforms/linux-openeuler.md)：源码构建兼容指南
- [Upgrade Guide](docs/release/upgrade.md)：升级、回滚和缓存清理
- [Smoke Matrix](docs/release/smoke-matrix.md)：验证矩阵
- [Getting Started](docs/getting-started.md)：详细入门指南
- [English Reference](docs/README.en.md)：英文参考说明

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
    project-model/       Rust project model, symbols, imports, calls, graph output
    cangjie/             Cangjie project model, symbols, diagnostics, graph output
    cli/                 CLI entry, unified output, MCP server, language detection
  fixtures/
    rust/                Rust graph contract fixture
    cangjie/             Cangjie fixture
    call-resolution/     Rust call resolution fixture
    import-use/          Rust import fixture
    item-extraction/     Rust symbol extraction fixture
  docs/
    architecture/        架构和输出格式文档
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
