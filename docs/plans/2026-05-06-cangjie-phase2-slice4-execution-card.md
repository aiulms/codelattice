# Phase 2 Slice 4 Execution Card — tree-sitter Cangjie preflight / vendor gate

**日期：** 2026-05-06
**状态：** 待执行
**前置：** Slice 3（baseline project model output）✅

## 1. Scope

**本 slice 是 docs-only preflight，不做任何 vendoring 或编译。**

在 Slice 3 完成 project model 的基础上，下一刀需要 tree-sitter-cangjie 来提取 AST 级别的符号和引用。但 tree-sitter-cangjie 涉及：
- **大文件 vendoring**（parser.c ~152K 行，~4.7MB）
- **C 编译依赖**（需要 `cc` crate）
- **非 crates.io 发布的 grammar**（需从 GitNexus-RC vendor 或上游获取）
- **license 兼容性验证**（Mulan PSL v2.0）

本 slice 只做 gate preflight，不直接 vendor。

## 2. 调查范围

| 项目 | 内容 |
|------|------|
| 来源 | 上游 repo + 可用版本 |
| License | Mulan PSL v2.0 兼容性评估 |
| ABI | 与 Rust-core 现有 tree-sitter 0.26 的兼容性 |
| 编译方式 | `cc` crate 编译 C 源码的集成方案 |
| Feature gate | feature flag 设计，与现有 `tree-sitter-extraction` 的关系 |
| 风险评估 | 文件大小、编译时间、平台兼容性 |
| 替代方案 | 如果没有 tree-sitter-cangjie，Cangjie 支持能做到什么程度 |

## 3. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `docs/plans/2026-05-06-cangjie-phase2-slice4-vendor-gate.md` | 新建 | vendor gate / feasibility doc（本文件即为 gate output） |
| `docs/plans/README.md` | 编辑 | 更新 Slice 4 状态 |

**不再修改 Rust-core runtime 代码。本 slice 不写任何 `.rs` 文件。**

## 4. Forbidden

- 不 vendor parser.c / scanner.c 到 Rust-core
- 不修改 `crates/cangjie/Cargo.toml` 依赖
- 不修改 workspace `Cargo.toml`
- 不新增 crates
- 不新增 `build.rs`
- 不改 GitNexus-RC runtime / Tool / live repo
- 不做 tree-sitter 编译测试（留待用户 approve 后）

## 5. Expected Output

一份 vendor gate 文档，包含：

1. **上游来源审计**：repo URL、commit、维护者、版本状态
2. **License 评估**：Mulan PSL v2.0 与项目兼容性
3. **ABI 兼容性分析**：tree-sitter-cangjie ABI 14 vs Rust-core tree-sitter 0.26
4. **编译方案设计**：`cc` crate + feature gate 的集成方式
5. **Feature gate 设计**：与现有 `tree-sitter-extraction` 的共存策略
6. **风险评估**：文件大小、编译时间、平台兼容性
7. **替代方案评估**：无 tree-sitter 时的 Cangjie 能力上限
8. **决策点**：给用户的选择（vendor / submodule / 等待上游 crates.io 发布 / 不做）

## 6. Stop-line

与 Slice 1-3 相同：
- 不实现代码
- 不 vendor 文件
- 不修改 Cargo.toml
- 不改 GitNexus-RC runtime
- 不改 Tool / live repo

## 7. Verification

- Rust-core: `cargo fmt --check` + `cargo check` + `cargo test` 应保持 123/123 pass（本 slice 不写代码）
- GitNexus-RC: `git diff --check` clean（本 slice 只改 Rust-core docs）

## 8. Comment Policy

本 slice 为 docs-only，不产生代码注释。
