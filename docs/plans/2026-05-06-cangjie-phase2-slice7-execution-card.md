# Phase 2 Slice 7 Execution Card — Cangjie graph output

**日期：** 2026-05-06
**状态：** 进行中
**前置：** Slice 7 preflight（推荐方案 B2）✅

## 1. Scope

基于 Slice 7 preflight 推荐方案 B2，在 cangjie crate 内实现独立 graph output：
- 定义 Cangjie-specific graph node/edge 类型
- 实现 `emit_cangjie_graph()` 从 `CangjieProject` + `Vec<CangjieSymbol>` 产出图结构
- JSON schema 与 project-model `GraphOutput` 保持结构兼容

本 slice 不做：
- project-model ItemExtractor trait 集成
- diagnostics 节点/边
- CALLS / IMPORTS / EXTENDS / IMPLEMENTS 边
- CLI 集成
- 嵌套符号
- LanguageAdapter trait 设计

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/graph.rs` | 新建 | Graph 类型定义 + emit_cangjie_graph() |
| `crates/cangjie/src/lib.rs` | 编辑 | 新增 `pub mod graph` |
| `crates/cangjie/tests/graph_smoke.rs` | 新建 | Graph output smoke tests |
| `docs/plans/2026-05-06-cangjie-phase2-slice7-execution-card.md` | 新建 | 本文件 |

## 3. Forbidden

- 不改 project-model crate
- 不改 CLI crate
- 不改 Rust analysis
- 不改 GitNexus-RC runtime / Tool / live repo
- 不新增外部 crate 依赖（serde + serde_json 已在 cangjie 依赖中）
- 不新增 workspace 依赖
- 不实现 diagnostics 节点/边
- 不实现 CALLS/IMPORTS/EXTENDS/IMPLEMENTS edges
- 不修改 workspace Cargo.toml

## 4. Expected Public API

```rust
// crates/cangjie/src/graph.rs (新建)

/// Build graph output from Cangjie project model and per-file symbols.
/// Returns nodes (Repository/Package/SourceFile/Symbol) and edges
/// (ContainsPackage/OwnsSource/Defines).
#[cfg(feature = "tree-sitter-cangjie")]
pub fn emit_cangjie_graph(
    project: &CangjieProject,
    symbols_by_file: &HashMap<PathBuf, Vec<CangjieSymbol>>,
) -> CangjieGraphOutput;

/// Build project model, extract symbols, and emit graph in one call.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn inspect_cangjie_project(root: &Path) -> Result<CangjieGraphOutput, ...>;
```

## 5. Acceptance Criteria

- [ ] `cargo build` 成功（不启用 feature）
- [ ] `cargo build --features tree-sitter-cangjie` 成功
- [ ] `cargo test` 保持 135/135 pass（零回归）
- [ ] `cargo test --features tree-sitter-cangjie` 新增 graph smoke tests 通过
- [ ] Graph output 包含 Repository/Package/SourceFile/Symbol 节点 + ContainsPackage/OwnsSource/Defines 边
- [ ] 测试覆盖 cjpm-basic fixture 的 graph 输出
- [ ] `cargo fmt --check` clean
- [ ] `git diff --check` clean
- [ ] 零新增依赖

## 6. Stop-line

- 不实现嵌套符号提取
- 不实现符号引用/关系
- 不接入 project-model graph emitter
- 不改 project-model / CLI / GitNexus-RC runtime
- 不新增依赖
