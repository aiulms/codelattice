# C++ Phase A Graph Support — Preflight

> **日期：** 2026-05-15
> **状态：** Preflight
> **范围：** CodeLattice C++ Phase A 静态代码图谱支持

---

## 目标

为 CodeLattice 添加 C++ Phase A 支持，使 AI agent 和人类开发者能够：

- 在本地扫描 C++ 项目（`.cpp`/`.hpp`/`.cc`/`.cxx`/`.h`），获取结构化代码图谱
- 理解命名空间、类、结构体、方法、函数、构造/析构函数等符号层级
- 查看函数调用关系（带 confidence tier）和 `#include` 依赖
- 使用与 Rust / Cangjie / TypeScript 一致的 CLI 和 MCP 接口

---

## 范围

### In Scope

- **静态代码图谱**：基于 tree-sitter-cpp 解析源码，提取符号和关系
- **符号类型**：namespace、class、struct、method、function、constructor、destructor、enum / enum class、using alias、typedef、macro
- **关系类型**：DEFINES（符号定义）、CALLS（函数调用，带 confidence tier）、INCLUDES（头文件包含）
- **CLI 支持**：`codelattice analyze --language cpp`、`quality`、`summary`、`export_bridge`
- **MCP 支持**：`codelattice_analyze`、`codelattice_project_overview`、`codelattice_symbol_search`、`codelattice_symbol_context`、`codelattice_query_graph`、`codelattice_impact_preview`、`codelattice_changed_symbols`、`codelattice_production_assist`
- **质量门**：duplicate_nodes、duplicate_edges、dangling_source、dangling_target、deterministic、calls_endpoint_integrity

### Out of Scope

- 完整预处理（macro expansion、conditional compilation）
- 构建系统执行（CMake、Bazel、Make 等）
- compile_commands.json 解析
- 模板实例化与模板元编程分析
- 完整重载解析（overload resolution）
- 虚函数派发解析（virtual dispatch resolution）
- 类型推断与类型检查
- 替代 clangd / Language Server

---

## 方法

1. **复用 C Phase A 架构**：C++ 与 C 共享相同的 tree-sitter 提取层和 graph builder 模式
2. **C++-specific 语义扩展**：
   - 在 C 的 function / struct / enum / macro / include 基础上，增加 class、method、constructor、destructor、namespace、using alias、typedef、enum class
   - 支持 `Class::method()` 限定名解析
   - 支持构造函数初始化列表识别
3. **Confidence Tier 策略**：
   - same-file call：0.90
   - cross-file heuristic（同目录 / 同名头文件）：0.75
   - unknown / ambiguous：0.50
4. **质量门对齐**：复用 Rust / Cangjie 已有的通用质量门，C++ 不适用 `external_symbol_marking` 和 `synthetic_nodes`
5. **Feature Flag**：`tree-sitter-cpp`，与 `tree-sitter-cangjie`、`tree-sitter-arkts`、`tree-sitter-typescript` 保持一致的可选编译策略

---

## 已知限制

| 限制 | 说明 |
|------|------|
| 无完整预处理 | `#ifdef`、`#define` 等只做识别，不做展开；条件编译分支全部纳入图谱 |
| 无构建系统执行 | 不运行 CMake / Bazel / Make；不解析编译选项、宏定义、include path |
| 无 compile_commands.json | 不依赖 compile_commands.json；纯源码静态分析 |
| 无模板实例化 | 模板定义提取为符号，但不实例化、不分析模板参数推导 |
| 无完整重载解析 | 函数调用按名称匹配，不做参数类型匹配和重载选择 |
| 无虚函数派发解析 | 虚函数调用按静态类型解析，不做动态派发分析 |
| 非 clangd 替代 | 不提供 IDE 级别的跳转、补全、重构能力 |

---

## 验证计划

- [ ] C++ fixture 分析通过（`fixtures/cpp/portable-smoke`）
- [ ] 质量门全部通过（0 duplicate、0 dangling、deterministic）
- [ ] MCP tool 端到端验证（analyze / symbol_search / impact_preview）
- [ ] CLI `--format json` 和 `--format gitnexus-rc` 输出验证
- [ ] Release smoke 包含 C++ fixture

---

## 相关文档

- [MCP Contract](docs/architecture/mcp-v0-contract.md) — C++ Phase A 工具 schema 和限制说明
- [Unified Output Contract](docs/architecture/unified-output-contract.md) — C++ 在统一输出协议中的位置
- [README](../README.md) — C++ CLI 使用示例和已知边界
