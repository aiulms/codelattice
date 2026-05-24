# CodeLattice

> 中文 README 是本项目的权威介绍与维护基准；英文说明仅作为外部 beta 用户的参考入口。

CodeLattice 是一个 **本地代码智能引擎**：面向大型、遗留、复杂代码库做静态图谱分析，把源码中的项目结构、符号、调用关系、依赖边界、质量信号和诊断结论转成 AI 编程助手与工程工具都能稳定消费的本地上下文。

它适合在接手陌生大仓、维护 legacy codebase、梳理 tangled dependencies、评估 high-risk change areas 或做代码审查前使用。CodeLattice 默认只读扫描源码，不上传代码，不执行项目构建脚本，用可复跑的图谱、质量门和影响分析帮助 AI 与人先读懂项目，再决定怎么改。

一句话概括：**先把代码地图画出来，再让 AI 下手。**

CodeLattice 用 Rust 编写，当前 beta 支持 Rust、Cangjie / 仓颉、ArkTS、TypeScript、C、C++、Python、Shell 八条本地图谱分析路径，并提供 CLI 与 MCP sidecar 两种使用方式。当前 master 在 `full` 模式保留 49 个 MCP 工具；默认 `ai` 模式只暴露 6 个 facade-first 入口，避免 AI 被底层工具选择题淹没。能力已经从“图谱查询”扩展到死代码候选、影响面分析、风险热点、架构偏移、可达性、公开 API 风险、框架入口提示、文档/测试/配置/自动化一致性审查、AI 工作流预设、工作区图谱、跨项目影响分析、证据驱动根因分析，以及面向增量分析的底层调度器基础。

**当前状态：外部 Beta / daily-use candidate（当前 master 发布目标为 `v0.16.0-beta.1`，最新已发布 GitCode Release 为 `v0.16.0-beta.1`）**。本地生产试用与 release smoke 已通过，但还不是 GA。CLI 输出、MCP contract、诊断结论和质量门在 beta 阶段仍可能以兼容优先的方式演进。完整变更见 [CHANGELOG](CHANGELOG.md)，验证矩阵见 [Smoke Matrix](docs/release/smoke-matrix.md)。

英文参考页：[docs/README.en.md](docs/README.en.md)

## 项目介绍

CodeLattice 的目标不是做一个托管索引服务，也不是替代编译器、IDE 或语言服务器。它提供的是一个可嵌入、可验证、可自动化的本地代码智能底座：

- **结构化理解**：识别项目、包、目标、源码文件、函数、类型、接口、宏、初始化函数等符号和所有权关系。
- **调用与依赖图谱**：抽取同文件、跨文件、导入绑定、有限关联函数、有限 receiver method、include/import 关系等静态边。
- **AI 代码审查上下文**：默认从 `codelattice_workflow` 意图路由器进入，再按 `nextActions` 调用 symbol context、calls_from / calls_to、impact_preview、changed_symbols、review_plan、production_assist、review_gate 等 MCP 工具。
- **场景化图谱诊断**：把现有图谱能力包装成 dead-code candidates、impact analysis、risk hotspots、architecture drift、reachability map、external API surface、framework entry hints 等可审查诊断结果。
- **一致性与兼容风险审查**：静态检查文档、测试、配置、示例和公开 API 变化，辅助 before_edit / after_edit / release_check 工作流。
- **证据驱动根因分析**：根据 AI 当前已有权限，结合代码图谱、git diff、日志/trace/HTTP/debug endpoint/浏览器等可用证据，输出候选根因、缺失证据、最小取证动作、可能修复区域和验证建议。
- **质量门与证据**：输出 dangling edge、重复节点、低置信度边、diagnostics、qualityMetrics 和 real-project corpus baseline。
- **增量调度基础**：用 `AnalysisRequest`、filesystem fingerprint、phase plan 和 cache decision 描述分析作业，为后续 daemon/watch/incremental 分析打底。
- **本地安全边界**：默认只读分析，不上传代码，不执行目标项目 build/test/package scripts。

换句话说，CodeLattice 把“代码长什么样、谁调用谁、改动可能影响哪里”这类上下文，从临时 grep 和人工猜测，变成稳定、可复跑、可审计的本地数据。

## 为什么适合研究大型遗留代码

很多真实项目并不来自干净的架构起点：文件很大、调用链很长、模块边界模糊、历史抽象层层叠加、文档和实现不一致。CodeLattice 的价值不是承诺一键重构，而是先把这些复杂系统变成 AI 和人都能审阅的工程地图。

- **read-first / review-first**：先定位入口点、热点文件、风险符号、低置信度区域，再进入修改。
- **impact analysis**：在改动前查看直接调用方、跨文件影响、风险理由和需要人工复核的边。
- **quality gates**：用 dangling edge、统计一致性、低置信度边和 diagnostics 作为 release / review 前置检查。
- **AI sidecar 友好**：MCP 输出带 `id / file / line / confidence / reason`，比让 AI 临时搜索一堆文件更适合安全改动。
- **重复分析可控**：Rust 本地引擎 + memory/persistent cache，适合在真实 AI 工作流里反复查询大仓。
- **调度过程可解释**：cache/prewarm 输出会暴露 scheduler phase、fingerprint 和 fresh/reuse decision，方便判断为什么复用或重跑。

所以 CodeLattice 特别适合接手陌生大仓、研究高耦合模块、拆解历史包袱重的代码、评估重构风险，以及让 AI 在复杂项目里“先问图谱，再改代码”。

## 为什么用 Rust

CodeLattice 选择 Rust 不是包装层面的卖点，而是产品能力的一部分：

- **本地分发简单**：适合构建单文件二进制和轻量 sidecar，便于接入 Codex、opencode、Claude Desktop 等本地工作流。
- **高频扫描更稳**：符号抽取、调用分析、图谱构建和缓存复用需要频繁扫描源码；Rust 的性能与内存模型适合这类本地分析核心。
- **安全边界清楚**：默认只读，不执行项目脚本；显式错误处理让工具行为更可预期。
- **验证成本更低**：确定性输出、quality gates、confidence / reason、fixture smoke 和 release smoke 都能固化进自动化测试。
- **跨平台路径明确**：当前以 macOS Apple Silicon 发行包和 Linux / openEuler 源码构建验证为主，后续扩多平台 artifact 路线清晰。

## 相比类似工具的优势

| 对比对象 | CodeLattice 的差异 |
|----------|--------------------|
| 云端代码智能服务 | 本地运行，默认不上传代码，适合私有仓库和受控环境 |
| 纯 IDE / LSP 能力 | 输出面向自动化消费，不绑定某个编辑器，可通过 CLI / MCP 接入多种工具 |
| grep / ripgrep / ctags | 不只做文本命中，还生成项目模型、符号图、调用边和质量报告 |
| 编译器或完整静态分析器 | 不追求完整类型推断、trait solving 或宏展开，而是提供工程上可解释、可标注置信度的快速上下文 |
| 通用多语言扫描器 | 对 Rust 与 Cangjie 有更深的项目模型、fixture、质量 gate 和调用策略；对 ArkTS / TypeScript / C / C++ / Python / Shell 提供 beta 静态图谱 |
| AI 插件内置索引 | CodeLattice 是独立本地核心，可复用、可测试、可离线、可审计 |

这也是项目名字里 “Lattice” 的含义：把分散的符号、文件、调用、质量信号和变更影响织成一个可查询的局部结构，让 AI 不只是“读一段代码”，而是能沿着工程关系理解项目。

## 支持语言

| 语言 | Beta 状态 | Fixture smoke | 主要支持 | 已知限制 |
|------|-----------|---------------|----------|----------|
| Rust | Stable | ✅ | Cargo 项目模型、符号、imports、CALLS、quality gates | 不做完整类型推断 / trait solving / macro expansion |
| Cangjie / 仓颉 | Stable | ✅ | cjpm 项目模型、符号、跨文件引用、调用、diagnostics | 不替代 cjc / cjlint |
| ArkTS / HarmonyOS | Production Trial | ✅ | HarmonyOS 项目识别、component/buildMethod、UI call extraction | 不完整解析 ArkUI DSL，不支持所有装饰器语义 |
| TypeScript | Beta hardened | ✅ | 符号、imports、calls、tsconfig paths、workspace package import | 不等同 tsc，不做类型系统求值 |
| C | Phase A hardened | ✅ | 符号、includes、compile_commands include path、qualityMetrics | 不做完整预处理器、宏展开或函数指针解析 |
| C++ | Phase A hardened | ✅ | 符号、includes、calls、compile_commands include path | 不做模板实例化、重载解析、虚调用解析 |
| Python | Phase A hardened | ✅ | 符号、calls、package import、relative import、re-export | 不执行代码，不解析动态 import / monkey patch |
| JavaScript | Phase A | ✅ | JS/JSX/MJS/CJS 符号、ESM import/export、CommonJS require/module.exports、package.json 入口 | 静态分析，不执行代码；dynamic import/require 为 diagnostic；不索引 node_modules |
| Shell | Phase A | ✅ | 脚本文件、函数、source 关系、命令调用、环境变量、风险诊断 | 不执行脚本，不替代 shellcheck，不展开复杂参数/条件 |

## 快速开始

### 1. 克隆

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice
```

### 2. 构建或打包 beta

开发构建：

```bash
bash scripts/install-mcp.sh --build
```

生成 release tarball：

```bash
scripts/package-release.sh
```

默认 release build 会启用 Cangjie / ArkTS / TypeScript / JavaScript / C / C++ / Python 全语言 feature；Shell 支持是轻量静态扫描路径，默认随 CLI 构建。

GitCode Release 页面发布后，也可以用安装器下载 tarball：

```bash
scripts/install-release.sh --version v0.16.0-beta.1 --install-dir /path/to/CodeLattice-Tool
```

### 3. 自检

```bash
scripts/codelattice-mcp.sh --self-test
scripts/release-smoke.sh --tarball dist/codelattice-0.16.0-beta.1-darwin-arm64.tar.gz
```

外部 fresh clone 路径：

```bash
scripts/fresh-clone-smoke.sh --skip-tests
```

这些脚本不会修改 Codex / opencode / Claude 的真实配置。

### 4. 分析 fixture

```bash
target/release/codelattice analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

### 5. 配 MCP

Contributor / debug 使用仓库内 wrapper：

```bash
scripts/codelattice-mcp.sh --self-test
```

普通 beta 用户推荐先 promote 到稳定目录：

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

打印客户端配置片段：

```bash
scripts/install-mcp.sh --print-config --install-dir "$CODELATTICE_TOOL_DIR"
```

配置中的 command 推荐使用参数化稳定路径：

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "/path/to/CodeLattice-Tool/codelattice-mcp.sh",
      "args": []
    }
  }
}
```

脚本只打印配置，不会自动写入真实 AI client。

## 当前限制

- CodeLattice 不是编译器、IDE、语言服务器或托管代码平台。
- 不保证完整类型推断，不做 Rust trait solving，不做 C/C++ 完整预处理器，不做宏展开。
- 不执行目标项目代码、构建脚本、测试脚本或 package scripts。
- 不上传代码，不依赖云端索引。
- MCP 和 CLI 输出在 beta 阶段仍可能演进；生产环境建议 pin 到具体版本并跑 self-test / release smoke。

## 兼容说明

Cargo package 和兼容二进制仍保留历史名称 `gitnexus-rust-core-cli`，用于已有脚本迁移。新的外部命令名推荐使用 `codelattice`。历史治理关系和旧名只在兼容说明、迁移文档和历史计划中保留，不作为对外产品主叙事。

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

### 分析 C 项目

```bash
target/release/codelattice analyze \
  --root /path/to/c/project \
  --language c \
  --format json
```

导出 GitNexus-RC bridge 格式：

```bash
target/release/codelattice analyze \
  --root /path/to/c/project \
  --language c \
  --format gitnexus-rc
```

### 分析 C++ 项目

```bash
target/release/codelattice analyze \
  --root /path/to/cpp-project \
  --language cpp \
  --format json
```

导出 GitNexus-RC bridge 格式：

```bash
target/release/codelattice analyze \
  --root /path/to/cpp-project \
  --language cpp \
  --format gitnexus-rc
```

### 分析 ArkTS / HarmonyOS 项目

```bash
target/release/codelattice analyze \
  --root /path/to/arkts/project \
  --language arkts \
  --format json
```

### 分析 Python 项目

```bash
target/release/codelattice analyze \
  --root /path/to/python-project \
  --language python \
  --format json
```

导出 GitNexus-RC bridge 格式：

```bash
target/release/codelattice analyze \
  --root /path/to/python-project \
  --language python \
  --format gitnexus-rc
```

### 分析 Shell 脚本目录

```bash
target/release/codelattice analyze \
  --root /path/to/scripts \
  --language shell \
  --format json
```

Shell 分析只做静态扫描：识别 `.sh/.bash/.zsh/.ksh/.bats` 和 shebang 脚本，抽取函数、`source` / `.` 引用、外部命令、环境变量读写，以及 `rm -rf`、`curl | sh` 等需要人工复核的风险模式。

导出 GitNexus-RC bridge 格式：

```bash
target/release/codelattice analyze \
  --root /path/to/scripts \
  --language shell \
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
- 找到 `.c`/`.h` 文件且无 C++ 文件：无 C++ 的 C 项目
- 找到 `.cpp`/`.hpp`/`.cc`/`.cxx` 文件：C++ 项目
- 找到 `pyproject.toml`/`setup.py`/`setup.cfg`/`requirements.txt` 或 `.py` 文件：Python 项目
- 找到 `.sh`/`.bash`/`.zsh`/`.ksh`/`.bats` 或 shell shebang 脚本，且没有更强语言清单：Shell 脚本项目
- 同时检测到多个语言：需要显式传入 `--language`

### 提交前变化审查

`detect-changes` 是 CodeLattice 自己的提交前变化审查入口，用来替代日常依赖外部 GitNexus-Tool 的 `detect-changes` 流程。它会基于 git diff 自动识别变更文件、变更符号、unknown hunks，并复用本地 `changed_symbols` / `production_assist` 能力生成风险摘要和 review checklist。同时自动检测 workspace 结构，提供文件归属映射、跨项目影响分析和不支持语言边界检测。

```bash
target/release/codelattice detect-changes \
  --root /path/to/git/repo \
  --language rust \
  --scope all
```

常用范围：

- `--scope all`：对比 `HEAD`，覆盖 staged + unstaged 变化
- `--scope staged`：只看已暂存变化
- `--scope unstaged`：只看未暂存变化
- `--base-ref <ref>`：与指定 git ref 对比

输出为 `codelattice.detectChanges.v1` JSON，包含 `changedFiles`、`changedSymbols`、`unknownHunks`、`risk`、`reviewChecklist`、`generatedFrom`，以及 workspace 相关字段：`workspaceContext`（workspace 检测结果）、`fileOwners`（每个变更文件的子项目归属）、`affectedProjects`（受影响的跨项目节点）、`affectedWorkspaceEdges`（受影响的 workspace 边）、`unsupportedBoundaryHits`（不支持语言边界命中）、`crossProjectRisk`（跨项目风险等级）、`recommendedFollowups`（推荐跟进项）。风险等级使用三层叠加：max(production_assist_risk, changed_symbol_risk, workspace_risk)。

为避免提交前漏掉新文件，`--scope all` 还会额外读取 `git ls-files --others --exclude-standard`，在 `untrackedFiles` 和 `summary.untrackedFileCount` 中报告未跟踪文件。

如果你在 CodeLattice 本仓开发，推荐直接运行原生 precommit bundle：

```bash
scripts/codelattice-precommit-check.sh
```

它会按顺序运行格式检查、diff whitespace 检查、productization/MCP regression、`codelattice-detect-changes` smoke，并最后输出本仓 `detect-changes` 摘要。默认不调用 GitNexus-Tool；旧 Tool 只作为过渡期 fallback 或对照检查。

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

AI 客户端日常配置不要设置 `CODELATTICE_MCP_TOOLSET=full`。推荐直接指向稳定 wrapper：

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh"
    }
  }
}
```

默认 MCP 工具面：

| 工具 | 什么时候用 |
|------|------------|
| `codelattice_workflow` | 不确定该用哪个工具时先用它；它会把意图路由成下一步可调用动作 |
| `codelattice_project` | 项目概览、质量门、热点、阅读路径、AI 上下文 |
| `codelattice_symbol` | 找符号、看上下文、查 callers/callees、局部图 |
| `codelattice_change_review` | 改动前后影响、删代码、发布检查、文档/测试/配置一致性、根因分析 |
| `codelattice_workspace` | 多项目/大仓根目录、跨项目图、跨项目影响 |
| `codelattice_cache` | 查看、解释、清理 CodeLattice 缓存 |

### AI 工作流指南

CodeLattice MCP 默认使用 `ai` toolset，只暴露上面 6 个入口工具。底层 49 个工具没有删除，只在显式调试模式中开放：

```bash
CODELATTICE_MCP_TOOLSET=core   # 常用底层工具 + facade
CODELATTICE_MCP_TOOLSET=full   # 全部 49 个工具，适合调试/回归 smoke
```

如果 Claude / OpenCode / TRAE 等日常 AI 客户端配置了 `CODELATTICE_MCP_TOOLSET=full`，模型会看到旧底层工具，容易绕开 facade/job/paging 保护。大项目和 monorepo 应优先使用 `codelattice_workspace mode=job → job_status → job_detail`。

外部用户和执行 AI 可以直接使用这些指南：

- [AI MCP Tool Guide](docs/guides/ai-mcp-tool-guide.md)：默认 6 个 MCP 的选择规则、模式表和示例调用。
- [AI Prompt Cookbook](docs/guides/ai-prompt-cookbook.md)：接手项目、改代码前后、删代码前、发布前、遗留代码清理等可复制提示词。
- [Workflow Presets](docs/guides/workflow-presets.md)：10 个场景对应的 MCP 工具链、关注字段和 stop-line。

这些工作流只组织静态分析工具，不会执行项目代码，也不会证明运行时行为、外部真实使用、测试覆盖率或删除安全性。

AI 编程助手推荐先调用 `codelattice_workflow`。它现在是意图路由器，会返回 `ai.workflow.v1` envelope：`situation`、`riskLevel`、`missingInputs`、`nextActions`、`cautions` 和 `safeToProceed`。`nextActions` 中的每一项都可以直接转成下一次 MCP `tools/call` 参数，减少 AI 猜工具和猜参数。

如果执行 AI 想要“一次调用先跑完常规检查”，可以传 `execute=true`。此时 workflow 会执行非递归的 nextActions，并返回 `execution`、`completedActions`、`failedActions`、`evidence` 和 `answerSummary`。如果缺少 `symbol` / `target` 等必要输入，执行会停在 `execution.status=needs_input`，不会盲目分析错误对象。

常见意图：

```json
{"mode":"onboarding","root":"/path/to/project","language":"auto"}
{"mode":"before_edit","root":"/path/to/project","language":"rust","symbol":"helper"}
{"mode":"before_edit","root":"/path/to/project","language":"rust","symbol":"helper","execute":true}
{"mode":"delete_code","root":"/path/to/project","language":"typescript","symbol":"oldApi"}
{"mode":"root_cause","root":"/path/to/project","language":"auto","issue":"dragging an object shows stale bounds after layout recompute","availableCapabilities":["read_code","read_git_diff","read_logs","local_http","edit_code"]}
{"mode":"cross_project_impact","root":"/path/to/workspace","target":{"path":"Dockerfile"}}
```

根因分析模式不会替 AI 修改代码或安装探针；它会先说明 AI 已经能看见什么，再给出当前最可能的根因假设、还缺什么证据、如果已有权限应优先自动读取什么，以及没有运行时入口时应加在哪些最小位置的临时探针。

如果缺少 `symbol` / `target` 等关键参数，`codelattice_workflow` 不会只返回失败，而是会在 `missingInputs` 中说明缺什么，并在 `nextActions` 里给出发现步骤，例如 `codelattice_symbol mode=search` 或 `codelattice_workspace mode=graph`。

AI 编程助手也可以使用这条 facade-first 链路完成“接手项目 → 改代码 → 看影响 → 审查 → 提交”的闭环：

1. `codelattice_workflow(mode=onboarding)`：选择接手项目的阅读路径和 stop-line
2. `codelattice_project(mode=overview|insights|ai_context|full, root=...)`：快速理解项目规模、热点、质量信号和 AI 编辑上下文
3. `codelattice_symbol(mode=search|context|callers|callees, root=..., name=...)`：定位符号、上下文和调用关系
4. `codelattice_change_review(mode=native_review|impact|full_review|safe_cleanup_review|release_check|root_cause, root=...)`：改动前后、删除、发布和根因路径统一走这个审查入口
5. `codelattice_workspace(mode=graph|impact, root=...)`：多项目仓库看跨项目关系和影响
6. `codelattice_cache(mode=status|explain, root=...)`：需要判断缓存复用或清理时使用

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
- **TypeScript**：支持 tsconfig path alias 与 workspace package import 的静态解析，但不运行 `tsc`，不提供类型系统保证
- **ArkTS**：`struct` 关键字由 tree-sitter-typescript 解析为 ERROR node，当前通过 pattern matching 恢复；暂不支持 `@Builder` / `@Extend`
- **C++**：Phase A 支持，可读取 compile_commands.json 做 include path resolution，但不做完整预处理、模板实例化、完整重载解析或虚函数派发解析；不是 clangd 的替代
- **Python**：Phase A 支持，不执行 Python 代码、不安装依赖、不读取虚拟环境、不做动态类型推断、不解析 eval/getattr/importlib 等动态调用、不替代 pyright/pylance/mypy
- **Shell**：Phase A 支持，只做静态脚本图谱和风险候选识别，不执行 shell，不解析复杂参数展开/条件执行/运行时 source 路径，不替代 shellcheck 或 CI 真实运行
- 不执行用户项目脚本
- 暂无 per-symbol incremental recompute，目前仍以项目级重新分析为主

## 安全模型

- 默认本地运行，不上传项目代码
- MCP `tools/list` 会为每个工具声明 `annotations` 与 `x-codelattice-permissionProfile`，方便 AI 客户端区分只读、cache 写入、`/tmp` artifact 写入和 smoke/debug 工具
- MCP sidecar 不写用户源码；`rename_preview` 只预览，不写文件
- `export_bridge` 只写入 `/tmp`
- `install-mcp.sh --print-config` 只打印配置模板，不修改 Codex / opencode / Claude 配置
- `fresh-clone-smoke.sh` 默认使用 `/tmp` 临时目录，并在结束后清理

## 项目状态与路线图

**外部 Beta / daily-use candidate（当前 master 发布目标为 `v0.16.0-beta.1`，最新已发布 GitCode Release 为 `v0.16.0-beta.1`）**：本地生产试用与 release smoke 已通过，但不是 GA。

当前相对可靠：

- Rust / Cangjie CLI 分析（Stable）
- ArkTS CLI 分析（Production Trial）
- TypeScript CLI 分析（path alias / monorepo import hardened）
- C CLI 分析（Phase A hardened）
- C++ CLI 分析（Phase A）
- Python CLI 分析（Phase A）
- JavaScript CLI 分析（Phase A）
- Shell CLI 分析（Phase A）
- MCP sidecar 默认 AI toolset 只暴露 6 个入口工具；`CODELATTICE_MCP_TOOLSET=full` 暴露 49 个底层/专家工具，覆盖图谱查询、诊断、审查、自动化图谱、AI 工作流预设、工作区图谱、跨项目影响分析和证据驱动根因分析
- 两层持久化缓存
- stable runtime promote
- release tarball packaging + release smoke
- fresh clone smoke
- 本地 AI client 集成模板

正在改进：

- Linux、Windows 等多平台 release 包
- Linux / openEuler native smoke certification
- 自动化 release CI
- diagnostics report / dashboard 形态
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

## WebUI — Snapshot Viewer

> **状态：** Phase I — Project Picker · One-Click Analyze · 中文/English

CodeLattice 提供了一个**纯静态本地 Web 页面**——Snapshot Viewer。它加载 `webui-snapshot.sh` 生成的 enriched JSON snapshot 并渲染为人类可浏览的 6 视图界面。

**Phase A 亮点：**
- 从 CLI analyze 输出中提取 **真实符号列表 + 源文件索引**
- Heuristic **cleanup 摘要**（死代码候选、不可达符号、外部 API surface）
- **10 个 workflow preset** 推荐（onboarding/before_edit/release_check 等）
- 多语言 fixture snapshot 矩阵：Rust / TypeScript / C / C++ / Python / Shell

### 快速开始

**Step 1: 生成 Enriched Snapshot（默认启用 --full）**

```bash
bash scripts/webui-snapshot.sh \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --output /tmp/codelattice-snapshot.json
```

**Step 2: 打开 Viewer**

```bash
open webui/snapshot-viewer/index.html
```

然后在页面中点击 **Load Snapshot** 按钮选择生成的 JSON 文件。

### 新增参数（Phase A）

```bash
--full                 # 启用全部 enrichment [默认]
--include-explore      # 提取符号/源文件数据
--include-review       # 提取 cleanup/release 摘要
--include-workflows    # 嵌入 workflow preset 推荐
--redact-root          # 脱敏绝对路径（用于 fixture）
--no-enrichment        # 回退到 MVP 最小 snapshot
```

### Smoke 验证

```bash
bash scripts/webui-snapshot-smoke.sh --full     # 生成并验证 6 语言 matrix
bash scripts/webui-viewer-smoke.sh              # viewer 结构验证 (35+ checks)
```

### WebUI Snapshot Viewer 功能

| 视图 | 内容 | 数据来源 |
|------|------|----------|
| **Dashboard** | 项目统计、Quality Gates (passed/failed)、Limitations | summary + quality + limitations |
| **Explore** | Source Files 列表、Symbols 搜索/过滤/排序、详情面板 | explore.symbols[] + sourceFiles[] |
| **Graph** | AntV G6/SVG 图谱、布局模板、下探、海报模式 | graph.* |
| **Cleanup** | Dead Code / Reachability / External API / Framework Hints + cautions | cleanup.* (heuristic) |
| **Release Review** | Breaking Change Risk / Doc Stale / Config Issues / Automation Graph + release cautions | releaseReview.* + automationGraph |
| **Workflows** | 10 个场景预设（工具推荐 + stop-lines）和自动化图谱审查 | workflowPresets + automationGraph |
| **🆕 Workspace** | 工作区发现：多项目扫描、语言分布、支持/暂不支持模块识别、批量分析、聚合摘要 | `/api/workspace/inventory` + `/api/workspace/analyze` |

### Runner 模式（本地分析工作台）

```bash
bash scripts/webui-runner.sh --open
```

启动后浏览器可：
- 直接分析单个项目
- **直接选择大目录 / workspace 根目录**：如果 `auto` 检测到多个可分析子项目，Runner 会自动进入 Workspace 模式并分析推荐项目，不再要求用户先猜具体子目录
- 分析推荐项目（一键）或勾选子项目批量分析；完成后停留在 Workspace 总览，不会自动把你带走
- 查看 Workspace 分析历史、每个子项目状态和 snapshot；洞察推荐项可一键打开对应子项目快照
- 暂不支持的语言（C#、Java、Go、Swift、Kotlin）会标注为「暂不支持模块」，并汇总为未来语言支持 backlog
- 复制一段适合发给 AI 的工作区摘要，用于下一步审查/清理规划

**Workspace 扫描规则**：只读取目录结构和 manifest 文件名，不读取文件内容、不执行任何项目代码。上限 depth=5、entries=5000，超出后标记 `truncated=true`。

**Protected live root 规则**：对 live repo 根目录，低层单项目 `analyze` 仍保持保护性拒绝；但 `auto` 入口、workspace graph 和 cross-project impact 可以把根目录当作只读工作区入口，输出会标注 `liveRootProtected=true`、`runtimeVerified=false`、`scriptsExecuted=false`。

CLI / MCP 中也遵循同一规则：

```bash
# 多项目根目录会返回 codelattice.workspaceAutoEntry.v1
target/debug/codelattice analyze --root /path/to/workspace --language auto --format json

# AI 推荐入口：language=auto 时自动判断单项目 vs workspace
codelattice_project(mode=overview, root=/path/to/workspace, language=auto)
```

### Multi-Language Fixture Snapshot Matrix

| 语言 | Snapshot | Symbols | Source Files | Status |
|------|----------|---------|-------------|--------|
| Rust | ✓ | 9 | 2 | PASS |
| TypeScript | ✓ | 20 | 4 | PASS |
| C | ✓ | 22 | 3 | PASS |
| C++ | ✓ | 33 | 3 | PASS |
| Python | ✓ | 23 | 5 | PASS |
| Shell | ✓ | 11 | 4 | PASS |

### 文档

| 文档 | 内容 |
|------|------|
| [docs/webui/README.md](docs/webui/README.md) | WebUI 总览、Phase A 架构 |
| [docs/webui/webui-mvp.md](docs/webui/webui-mvp.md) | MVP/Phase A 视图规格 |
| [docs/webui/webui-snapshot-contract.md](docs/webui/webui-snapshot-contract.md) | `CodeLatticeWebSnapshotV1` JSON contract |
| [webui/snapshot-viewer/README.md](webui/snapshot-viewer/README.md) | Viewer 使用指南 |

### 硬边界

本轮包含：纯静态 HTML/CSS/JS viewer、snapshot 聚合脚本 (Python)、多语言 fixture matrix。
本轮不包含：前端框架、后端服务、MCP 直连、桌面应用壳。

## License

MIT License. See [LICENSE](LICENSE).
