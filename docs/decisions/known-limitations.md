# 已知局限

> **日期：** 2026-08-01
> **类型：** limitation inventory
> **状态：** 已填充（与 AGENTS.md stop-lines / mcp-v0-contract §八 对齐）

---

## 目的

记录 CodeLattice 分析引擎与 AI 工具面的已知局限。本文件是 AI 客户端（agent）与人类审查者共同的「不能做什么」权威清单；AI 不应声称拥有下列能力。

---

## 引擎级 Stop-lines（MVP 明确不支持）

- **无完整类型推断 / trait solving** — 不推断变量类型，不做 trait bound satisfaction。
- **无宏展开** — `foo!()` 不展开。
- **无完整 cfg evaluator** — cfg-gated `mod` 只标记 `unknown`。
- **无 `cargo metadata` 执行** — 只用 manifest-derived project model。
- **无 proc-macro / build.rs 执行**。
- **外部 crate 支持有界** — 允许 std/core/alloc 直接路径和导入的 stdlib/prelude 类型有限解析；禁止任意 external crate API 深度解析。
- **Method dispatch 是低置信度启发式** — 允许 blind method-name / explicit receiver-type annotation 启发式；禁止 full receiver type inference / trait solving。
- **不执行目标项目代码** — 不运行 build/test/package scripts，不执行 shell、Python、JS。
- **不上传代码** — 本地只读分析，无云端索引依赖。

## 语言级已知边界

| 语言 | 边界 |
|------|------|
| Rust | 不做完整类型推断 / trait solving / macro expansion；调用边是带 confidence/reason 的启发式结果，不是编译器证明 |
| Cangjie | 不替代 cjc / cjlint；无完整 method dispatch / 类型推断 / interface solving / 宏展开 / cfg 求值 |
| ArkTS | 不完整解析 ArkUI DSL；`struct` 由 tree-sitter-typescript ERROR node 模式恢复；不支持 `@Builder` / `@Extend` |
| TypeScript | 不等同 tsc；不做类型系统求值；支持 tsconfig path alias / workspace package import 静态解析 |
| C | 不做完整预处理器、宏展开或函数指针解析 |
| C++ | 不做模板实例化、完整重载解析、虚调用解析；不是 clangd 替代 |
| Python | 不执行代码、不装依赖、不读虚拟环境；不做动态类型推断；不解析 eval/getattr/importlib；不替代 pyright/pylance/mypy |
| JavaScript | 不执行代码；dynamic import/require 为 diagnostic；不索引 node_modules |
| Shell | 不执行脚本、不替代 shellcheck；不解析复杂参数展开/条件执行/运行时 source 路径 |

## MCP / AI 工具面局限

- **无 streaming** — 所有 `tools/call` 都是完整响应后返回。
- **单 root 语义** — 每个调用针对一个 project root；workspace 根需走 `codelattice_workspace` 或 auto-entry。
- **符号搜索是 substring 匹配**，不是语义搜索。
- **图遍历深度有界** — BFS 默认 maxDepth 3。
- **无 per-symbol incremental recompute** — 当前以项目级重新分析为主；scheduler 只产生 `incrementalPlan` 元数据（`plan_only=true`），缓存 miss 时仍执行全量分析。
- **缓存键粒度粗** — MCP 缓存键为 `{root, language, strict}`；任意源文件变化会使整个项目级 artifact 失效。
- **静态结果不等于运行证明** — 静态分析无法证明运行时行为、测试覆盖率、真实外部使用、删除安全性。
- **`full` toolset 是调试面** — 默认 `ai` toolset 只暴露 6 个 facade 工具；`full`（50 个工具）用于调试/回归，日常 AI 客户端不应启用。

## 降级行为

- 遇到不支持场景（未知语言、动态调用、宏调用、外部 crate 深度解析等）时：
  - 产生 no-edge / low-confidence 结果，或标记 `unknown`，而不是伪造证据。
  - 输出带 `confidence` / `reason` / `cautions` / `generatedFrom(staticAnalysis=true, targetCodeExecuted=false)` 字段，AI 可据此判断可信度。
  - 缓存不可写时静默回退 memory-only；`CODELATTICE_CACHE=off` 可完全关闭。

---

## 来源

- [AGENTS.md](../../AGENTS.md) — stop-lines 与 active bug gate
- [mcp-v0-contract.md §八 Known Limitations](../architecture/mcp-v0-contract.md) — MCP 契约层局限
- [README.md 已知边界](../../README.md) — 语言级边界
- [no-edge-policy.md](./no-edge-policy.md) — no-edge over false-edge 策略
