# Phase 2 Slice 6 Execution Card — tree-sitter Cangjie AST symbol extraction

**日期：** 2026-05-06
**状态：** 完成 ✅
**前置：** Slice 5（tree-sitter 集成）✅

## 1. Scope

基于已验证的 tree-sitter-cangjie parser，使用 tree_sitter::Query 从 .cj 源文件提取 top-level 符号。

提取的符号类型（对齐 tags.scm 定义）：
- `functionDefinition` → function
- `classDefinition` → class
- `structDefinition` → struct
- `enumDefinition` → enum
- `interfaceDefinition` → interface
- `typeAlias` → typeAlias
- `macroDefinition` → macro

本 slice 不做：
- 完整的 project-model ItemExtractor trait 集成（留待 Slice 7）
- graph output
- diagnostics
- 嵌套符号（方法/属性/内部类）
- 符号引用/关系提取
- LSP

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/extractors/mod.rs` | 编辑 | 新增 CangjieSymbol/CangjieSymbolKind 类型 + extract_cangjie_symbols() |
| `crates/cangjie/src/extractors/symbol.rs` | 新建 | symbol 类型定义和 query 逻辑（从 mod.rs 抽出可选） |
| `crates/cangjie/tests/tree_sitter_smoke.rs` | 编辑 | 新增 symbol extraction 测试 |
| `docs/plans/2026-05-06-cangjie-phase2-slice6-execution-card.md` | 新建 | 本文件 |

## 3. Forbidden

- 不改 project-model crate
- 不改 CLI crate
- 不改 Rust analysis
- 不改 GitNexus-RC runtime / Tool / live repo
- 不新增 graph output
- 不实现 diagnostics
- 不实现嵌套符号提取
- 不修改 workspace Cargo.toml
- 不新增依赖

## 4. Expected Public API

```rust
// crates/cangjie/src/extractors/mod.rs (新增)

/// Top-level Cangjie symbol kinds
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CangjieSymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Interface,
    TypeAlias,
    Macro,
}

/// A top-level symbol extracted from a Cangjie source file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CangjieSymbol {
    pub kind: CangjieSymbolKind,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Extract top-level symbols from Cangjie source code using tree-sitter queries
#[cfg(feature = "tree-sitter-cangjie")]
pub fn extract_cangjie_symbols(source: &str) -> Result<Vec<CangjieSymbol>, CangjieParseError>;
```

## 5. Acceptance Criteria

- [x] `cargo build` 成功（不启用 feature）
- [x] `cargo build --features tree-sitter-cangjie` 成功
- [x] `cargo test` 保持 135/135 pass（已有测试零回归）
- [x] `cargo test --features tree-sitter-cangjie` 新增 symbol extraction 测试通过
- [x] 测试覆盖 7 种符号类型（function via func + main, class, struct, enum, interface, typeAlias）
- [x] 测试覆盖空源文件（空结果）
- [x] 已知限制：macro 函数定义语法不被当前 grammar 支持（已记录测试）
- [x] `cargo fmt --check` clean
- [x] `git diff --check` clean

## 6. Stop-line

- 不实现嵌套符号提取（类内方法等）
- 不实现符号引用/关系
- 不接入 graph emitter
- 不改 project-model / CLI / GitNexus-RC runtime
- 不新增依赖
