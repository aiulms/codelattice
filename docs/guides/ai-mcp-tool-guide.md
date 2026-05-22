# CodeLattice AI MCP Tool Guide

CodeLattice MCP 默认对 AI 客户端只暴露 6 个入口工具。目标是让执行 AI 不再面对几十个名字相近的底层工具，而是先按意图选择 facade，再由 facade 内部编排底层能力。

所有输出仍然是静态分析结果：不执行目标项目代码，不证明运行时行为，不证明测试覆盖率，也不证明删除安全。

## 默认 6 个工具

| 工具 | 主要用途 | 典型模式 |
|------|----------|----------|
| `codelattice_workflow` | 不知道该从哪里开始时使用；把人类意图路由成下一步动作 | `onboarding`, `before_edit`, `after_edit`, `delete_code`, `release_check`, `root_cause` |
| `codelattice_project` | 项目级理解、质量门、热点、AI 上下文 | `overview`, `quality`, `insights`, `ai_context`, `full` |
| `codelattice_symbol` | 查符号、看上下文、找 callers/callees、局部图 | `search`, `context`, `callers`, `callees`, `graph` |
| `codelattice_change_review` | 改动审查、影响分析、删代码审查、发布审查、根因假设 | `native_review`, `impact`, `full_review`, `safe_cleanup_review`, `release_check`, `docs_tests`, `config_examples`, `root_cause` |
| `codelattice_workspace` | 大仓/多项目入口、跨项目图、跨项目影响 | `overview`, `graph`, `impact`, `full` |
| `codelattice_cache` | 缓存状态、缓存解释、缓存清理 | `status`, `explain`, `clear` |

## 选择规则

1. 不确定：先调用 `codelattice_workflow`。
2. 接手项目：`codelattice_project mode=full`，多项目仓库先 `codelattice_workspace mode=overview`。
3. 找代码位置：`codelattice_symbol mode=search`，再 `mode=context`。
4. 改代码前：`codelattice_change_review mode=impact`，目标不明确时先 `codelattice_workflow mode=before_edit`。
5. 改代码后：`codelattice_change_review mode=native_review` 或 `mode=full_review`。
6. 想删代码：`codelattice_change_review mode=safe_cleanup_review`，不要只凭 dead-code 结果删除。
7. 发布/提测前：`codelattice_change_review mode=release_check`。
8. 文档/测试同步：`codelattice_change_review mode=docs_tests`。
9. 配置/示例同步：`codelattice_change_review mode=config_examples`。
10. 根因分析：`codelattice_change_review mode=root_cause`，或 `codelattice_workflow mode=root_cause`。

## 示例调用参数

```json
{"mode":"onboarding","root":"/path/to/project","language":"auto"}
{"mode":"full","root":"/path/to/project","language":"auto"}
{"mode":"search","root":"/path/to/project","language":"auto","query":"helper"}
{"mode":"impact","root":"/path/to/project","language":"auto","symbol":"helper"}
{"mode":"safe_cleanup_review","root":"/path/to/project","language":"typescript","symbol":"oldApi"}
{"mode":"release_check","root":"/path/to/project","language":"auto"}
{"mode":"root_cause","root":"/path/to/project","language":"auto","issue":"保存后页面仍显示旧数据","availableCapabilities":["read_code","read_git_diff","read_logs"]}
{"mode":"graph","root":"/path/to/workspace","compact":true}
{"mode":"impact","root":"/path/to/workspace","target":{"path":"Dockerfile"},"direction":"both","maxDepth":3}
```

## 隐藏工具怎么办

默认 `ai` toolset 会隐藏底层和专家工具，例如 `codelattice_dead_code_candidates`、`codelattice_release_check`、`codelattice_root_cause_assistant`。这些能力没有消失，只是统一收进 `codelattice_change_review`、`codelattice_project`、`codelattice_workspace` 等入口。

如果 AI 调用了隐藏工具，MCP 会返回 `tool_not_in_ai_toolset`，并提示应该使用哪个入口工具。高级调试或回归脚本可以显式打开：

```bash
CODELATTICE_MCP_TOOLSET=core   # 6 个入口 + 常用底层工具
CODELATTICE_MCP_TOOLSET=full   # 全部 49 个工具
```

## 推荐心智模型

把 CodeLattice 当成“代码地图和审查信号”的提供者：

- 它给出结构化图谱、调用链、影响范围、跨项目边界和静态风险提示。
- 它不会替代编译器、测试、运行时日志和人工判断。
- AI 应把结果当成调查线索和审查清单，而不是安全证明。

