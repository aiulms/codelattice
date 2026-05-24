# CodeLattice AI MCP Tool Guide

CodeLattice MCP 默认对 AI 客户端只暴露 6 个入口工具。目标是让执行 AI 不再面对几十个名字相近的底层工具，而是先按意图选择 facade，再由 facade 内部编排底层能力。

所有输出仍然是静态分析结果：不执行目标项目代码，不证明运行时行为，不证明测试覆盖率，也不证明删除安全。

## 默认 6 个工具

| 工具 | 主要用途 | 典型模式 |
|------|----------|----------|
| `codelattice_workflow` | 不知道该从哪里开始时使用；把人类意图路由成下一步动作 | `onboarding`, `before_edit`, `after_edit`, `delete_code`, `release_check`, `root_cause` |
| `codelattice_project` | 项目级理解、质量门、热点、AI 上下文；大项目可走 job | `overview`, `quality`, `insights`, `ai_context`, `full`, `job`, `job_status`, `job_detail` |
| `codelattice_symbol` | 查符号、看上下文、找 callers/callees、局部图；大项目可走 job | `search`, `context`, `callers`, `callees`, `graph`, `job`, `job_status`, `job_detail` |
| `codelattice_change_review` | 改动审查、影响分析、删代码审查、发布审查、根因假设；大项目可走 job | `native_review`, `impact`, `full_review`, `safe_cleanup_review`, `release_check`, `docs_tests`, `config_examples`, `root_cause`, `job`, `job_status`, `job_detail` |
| `codelattice_workspace` | 大仓/多项目入口、跨项目图、跨项目影响；大仓优先走 job | `overview`, `graph`, `impact`, `full`, `job`, `job_status`, `job_detail` |
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
11. 大项目/monorepo：优先提交 job，不要同步拉完整底层输出。

## 大项目 job 模式

大项目和 monorepo 不要让 AI 调用底层 `codelattice_project_overview` 或其他 full toolset 工具。正确流程是：

```json
{"mode":"job","root":"/path/to/workspace","language":"auto","compact":true}
{"mode":"job_status","jobId":"job_engine_00000001"}
{"mode":"job_detail","jobId":"job_engine_00000001","page":0,"pageSize":50}
```

单项目请使用明确项目根和明确语言：

```json
{"mode":"job","root":"/path/to/workspace/packages/app","language":"typescript","compact":true}
{"mode":"job_status","jobId":"job_engine_00000002"}
{"mode":"job_detail","jobId":"job_engine_00000002","page":0,"pageSize":50}
```

`job_status` 只需要 `jobId`。`job_detail` 只需要 `jobId`，以及可选的 `page` / `pageSize`。不要给这两个模式补一个猜测出来的 `root`。

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
{"mode":"job","root":"/path/to/workspace","language":"auto","compact":true}
{"mode":"job_status","jobId":"job_engine_00000001"}
{"mode":"job_detail","jobId":"job_engine_00000001","page":0,"pageSize":25}
```

## 隐藏工具怎么办

默认 `ai` toolset 会隐藏底层和专家工具，例如 `codelattice_dead_code_candidates`、`codelattice_release_check`、`codelattice_root_cause_assistant`。这些能力没有消失，只是统一收进 `codelattice_change_review`、`codelattice_project`、`codelattice_workspace` 等入口。

Claude、OpenCode、TRAE 等日常 AI 客户端不要设置 `CODELATTICE_MCP_TOOLSET=full`。默认 6 个 facade 是正确模式；`full` 只用于开发者调试、回归脚本和 dogfood。若 AI 客户端设置为 `full`，模型容易直接调用旧底层工具，例如 `codelattice_project_overview` / `codelattice_cache_status`，绕开 facade 的 workspace 诊断、compact 输出和 job 分页保护。

如果 AI 调用了隐藏工具，MCP 会返回 `tool_not_in_ai_toolset`，并提示应该使用哪个入口工具。开发者调试或回归脚本可以显式打开：

```bash
CODELATTICE_MCP_TOOLSET=core   # 6 个入口 + 常用底层工具
CODELATTICE_MCP_TOOLSET=full   # 全部 49 个工具
```

日常客户端配置应省略 `CODELATTICE_MCP_TOOLSET`，或显式设为 `ai`。修改配置或重新 promote `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` 后，需要重启 Claude/OpenCode/TRAE 的 MCP session，让客户端重新读取工具列表。

## Busy 提示怎么处理

`mcp_server_busy` 不是崩溃，也不是缓存锁死。它表示当前 MCP session 里还有一个 CodeLattice `tools/call` 没完成；server 会拒绝重叠调用，避免客户端把多个长调用堆在同一个 stdio session 里直到超时。

处理方式：

1. 等当前调用完成后再重试一次。
2. 不要在同一 session 中并发发多个 CodeLattice MCP tool call。
3. 如果一个调用结束后仍持续 busy，通常是客户端旧会话仍保留挂起请求；重启 MCP session 或重启客户端连接。
4. 大项目请用 `mode=job`、`job_status`、`job_detail`，避免长同步调用。

## 推荐心智模型

把 CodeLattice 当成“代码地图和审查信号”的提供者：

- 它给出结构化图谱、调用链、影响范围、跨项目边界和静态风险提示。
- 它不会替代编译器、测试、运行时日志和人工判断。
- AI 应把结果当成调查线索和审查清单，而不是安全证明。
