# CodeLattice 仓颉（Cangjie）支持指南

> 本文是仓颉开发者使用 CodeLattice 的入口文档。CodeLattice 是用 Rust 编写的本地代码图谱引擎，
> 对仓颉项目提供项目结构识别、符号提取、跨文件引用解析、调用关系图谱和质量门检查，
> 全程只读扫描源码，不执行 `cjpm build`，不上传代码。
>
> 仓颉路径的当前状态为 **Stable**（见 `docs/release-versioning.md` 语言支持表）：
> 已在真实项目上生产试用，质量门通过。本文所有输出均为
> CodeLattice `0.17.0-beta.2` 发布二进制对本仓 `fixtures/cangjie/portable-smoke`
> fixture 的真实运行结果，可复跑验证。

## 对仓颉项目提供什么

| 能力 | 说明 |
|------|------|
| 项目模型 | 识别 `cjpm.toml` 单包与 workspace 多包结构（`members`、嵌套包），输出 包 → 源文件 的所有权边 |
| 符号提取 | class / struct / enum / interface / func / init / 主函数等符号，含跨文件唯一 ID |
| 引用解析 | 跨文件 `import` 绑定（`import lib.types.{Point, Size}`）解析到真实符号，产出 import / uses 边 |
| 调用引用 | 构造调用、跨文件函数调用、方法调用产出 References/Uses 边，附 confidence 与 reason |
| 诊断与质量门 | 合成节点、重复节点/边、悬垂端点、确定性等结构质量门（见下文真实输出） |
| 消费方式 | CLI（文本/JSON）、`detect-changes` 提交前审查、MCP sidecar（供 Claude Desktop / Codex / opencode 等 AI 工具调用） |

## 安装

```bash
# 方式一：安装发布包（macOS Apple Silicon，当前 v0.17.0-beta.2）
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.17.0-beta.2 \
  --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test

# 方式二：源码构建（含仓颉适配需 --features tree-sitter-cangjie，打包脚本已默认启用）
git clone https://gitcode.com/aiulms/codelattice.git && cd codelattice
cargo build --release -p gitnexus-rust-core-cli \
  --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python
```

## 快速上手（真实输出走查）

以本仓自带的仓颉 fixture 为例（一个 `cjpm.toml` 包，`src/main.cj` 通过
`import lib.types.{...}` 与 `import lib.math.{...}` 引用两个库文件）：

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice
codelattice analyze --root fixtures/cangjie/portable-smoke --language cangjie --format json
```

摘要输出（`summary` 字段）：

```json
{
 "sourceFileCount": 3,
 "symbolCount": 22,
 "nodeCount": 27,
 "edgeCount": 36
}
```

结构边示例（`graph.edges` 节选，包所有权 + 文件归属）：

```json
{ "kind": "containsPackage", "sourceId": "repo:cangjie", "targetId": "pkg:portable-smoke" },
{ "kind": "ownsSource", "sourceId": "pkg:portable-smoke", "targetId": "file:src/lib/math.cj" }
```

节点类型分布：`repository ×1`、`package ×1`、`sourceFile ×3`、`symbol ×22`。

质量门结果（本 fixture 全部通过）：

```text
synthetic_nodes   => PASS   （无合成 CallableSource 节点）
duplicate_nodes   => PASS
duplicate_edges   => PASS
dangling_source   => PASS   （边端点不悬垂）
dangling_target   => PASS
deterministic     => PASS   （两次分析输出一致）
```

workspace 多包项目（`cjpm.toml` 声明 `members = ["pkg1", "pkg2"]`）使用同一命令，
项目/依赖结构见 `fixtures/cangjie/cjpm-workspace`。

## 在自己的仓颉项目上使用

```bash
# 全量图谱分析（JSON 供脚本消费，去掉 --format json 输出同构文本）
codelattice analyze --root /path/to/cangjie-project --language cangjie --format json

# 提交前变更审查：对 git 工作区改动输出影响面与风险等级
codelattice detect-changes --root /path/to/cangjie-project --language cangjie --compact

# 多项目仓库一次合并出图（Rust + 仓颉 + 前端混仓）
codelattice workspace --root /path/to/monorepo --compact
```

## 接入 AI 编程工具（MCP）

CodeLattice 默认以 MCP sidecar 形态服务 AI 工具，仓颉能力通过
`codelattice_project` / `codelattice_symbol` 等 facade 工具直接可用
（`initialize` 应答中 `cangjieSupport: true`）。以 Claude Desktop 为例：

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "/Users/you/.local/share/codelattice-tool/codelattice-mcp.sh",
      "env": { "CODELATTICE_MCP_TOOLSET": "ai" }
    }
  }
}
```

默认 `ai` 模式暴露 6 个稳定入口（workflow / project / symbol / change review /
workspace / cache）；`full` 模式提供 50 个工具用于调试与底层图谱查询。

## 已知边界（诚实声明）

- CodeLattice 是**静态分析**：不编译仓颉代码、不执行测试、不证明运行时可达性；
  结论是调查线索，不是编译器证明。
- 方法调用解析基于启发式（方法名唯一性 / 显式 receiver 类型），带 confidence 与
  reason 字段如实标注；不做完整 receiver 类型推断。低置信度解析按 no-edge 政策
  记诊断而非产出边。
- cfg/条件编译等语义不展开。完整边界见 `docs/decisions/known-limitations.md`。

## 相关链接

- 项目主页与文档：[README](../README.md) / [快速上手](getting-started.md)
- 当前版本说明：[0.17.0-beta.2 Release Notes](release/0.17.0-beta.2-notes.md)
- 仓颉社区收录：[awesome-cangjie 开发工具分类](https://github.com/gtn1024/awesome-cangjie)
