# Phase 2 Slice 5 Execution Card — tree-sitter Cangjie 集成

**日期：** 2026-05-06
**状态：** 完成 ✅
**前置：** Slice 4（vendor gate）✅ — 用户已通过 re-enter autonomous mode 批准 Option A

## 1. Scope

将 tree-sitter-cangjie vendor 到 cangjie crate，通过 `cc` crate 编译，配置 feature gate，实现最小 AST parse 验证。

本 slice 不做：
- 完整的 symbol extraction（留待 Slice 6）
- AST queries（留待 Slice 6）
- 任何 graph output

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/Cargo.toml` | 编辑 | 新增 tree-sitter + cc 可选依赖 + feature gate |
| `crates/cangjie/build.rs` | 新建 | cc::Build 编译 parser.c + scanner.c |
| `crates/cangjie/vendor/tree-sitter-cangjie/src/parser.c` | 新建 | 从 GitNexus-RC vendor copy |
| `crates/cangjie/vendor/tree-sitter-cangjie/src/scanner.c` | 新建 | 从 GitNexus-RC vendor copy |
| `crates/cangjie/vendor/tree-sitter-cangjie/src/tree_sitter/parser.h` | 新建 | 编译所需 header |
| `crates/cangjie/vendor/tree-sitter-cangjie/src/tree_sitter/alloc.h` | 新建 | 编译所需 header |
| `crates/cangjie/vendor/tree-sitter-cangjie/src/tree_sitter/array.h` | 新建 | 编译所需 header |
| `crates/cangjie/vendor/tree-sitter-cangjie/LICENSE` | 新建 | License 副本 |
| `crates/cangjie/vendor/tree-sitter-cangjie/README.OpenSource` | 新建 | 上游来源记录 |
| `crates/cangjie/src/extractors/mod.rs` | 新建 | tree-sitter-cangjie 语言加载 + smoke parse 函数 |
| `crates/cangjie/src/lib.rs` | 编辑 | `pub mod extractors` + 条件导出 |
| `docs/plans/2026-05-06-cangjie-phase2-slice5-execution-card.md` | 新建 | 本文件 |

## 3. Forbidden

- 不改 project-model crate
- 不改 CLI crate
- 不改 Rust analysis
- 不改 GitNexus-RC runtime / Tool / live repo
- 不新增 graph output
- 不实现 symbol extraction（只做 parse 验证）
- 不实现 AST queries
- 不修改 workspace Cargo.toml

## 4. Feature Gate 设计

```toml
[features]
default = []
tree-sitter-cangjie = ["dep:tree-sitter", "dep:cc"]

[dependencies]
tree-sitter = { version = "0.26", optional = true }

[build-dependencies]
cc = { version = "1", optional = true }
```

与 project-model 的 `tree-sitter-extraction` 完全独立。`tree-sitter-cangjie` 默认关闭。

## 5. Expected Public API

```rust
// crates/cangjie/src/extractors/mod.rs

/// 初始化 tree-sitter-cangjie parser（仅在 feature 启用时可用）
#[cfg(feature = "tree-sitter-cangjie")]
pub fn try_init_cangjie_parser() -> Option<tree_sitter::Parser>;

/// 检查 tree-sitter-cangjie 是否可用
pub fn is_cangjie_parser_available() -> bool;

/// 对 .cj 源码做最小 parse 验证（返回是否成功且无 ERROR）
#[cfg(feature = "tree-sitter-cangjie")]
pub fn parse_cangjie_source(source: &str) -> Result<tree_sitter::Tree, ParseError>;
```

## 6. Acceptance Criteria

- [x] `cargo build` 成功（不启用 feature，零新增编译）
- [x] `cargo build --features tree-sitter-cangjie` 成功编译 parser.c
- [x] `cargo test` 保持 126/126 pass（已有测试零回归）
- [x] `cargo test --features tree-sitter-cangjie` 新增 smoke test 通过
- [x] smoke test：parse `fixtures/cangjie/cjpm-basic/src/main.cj` 无 ERROR node
- [x] `cargo fmt --check` clean
- [x] `git diff --check` clean

## 7. Stop-line

- 不实现 symbol extraction / AST queries
- 不接入 graph emitter
- 不改 project-model / CLI / GitNexus-RC runtime
- Parser bug 已知（见 RISK_LEDGER §3.3），不在此轮修复
