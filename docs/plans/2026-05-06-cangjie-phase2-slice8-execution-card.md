# Phase 2 Slice 8 Execution Card — Cangjie diagnostics runner (cjc/cjlint subprocess)

**日期：** 2026-05-06
**状态：** 进行中
**前置：** Slice 8 preflight（推荐方案 A）✅

## 1. Scope

基于 Slice 8 preflight 推荐方案 A，在 cangjie crate 内实现 subprocess diagnostics runner：

- 新增 `diagnostics/` 模块（types.rs + runner.rs + mod.rs）
- SDK tool discovery（CANGJIE_HOME → CANGJIE_SDK_HOME → PATH）
- cjc runner：`cjc --diagnostic-format=json --output-type=staticlib <file>`
- cjlint runner：`cjlint -r json -o <tmpfile> <project_root>`
- 扩展 graph.rs：新增 Diagnostic NodeKind + Annotates EdgeKind + emit_cangjie_diagnostics()
- Graceful degrade：SDK 不可用时返回空 diagnostics，不崩溃

本 slice 不做：
- LSP client
- diagnostics auto-fix
- incremental diagnostics
- CLI flag
- 默认启用 diagnostics

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/diagnostics/mod.rs` | 新建 | 模块声明 + re-exports |
| `crates/cangjie/src/diagnostics/types.rs` | 新建 | CangjieDiagnostic + DiagnosticSeverity |
| `crates/cangjie/src/diagnostics/runner.rs` | 新建 | cjc/cjlint subprocess + SDK tool discovery + graceful degrade |
| `crates/cangjie/src/graph.rs` | 编辑 | 新增 NodeKind::Diagnostic + EdgeKind::Annotates + emit_cangjie_diagnostics() |
| `crates/cangjie/src/lib.rs` | 编辑 | 新增 `pub mod diagnostics` |
| `docs/plans/2026-05-06-cangjie-phase2-slice8-execution-card.md` | 新建 | 本文件 |

## 3. Forbidden

- 不改 project-model crate
- 不改 CLI crate
- 不改 Rust analysis
- 不改 GitNexus-RC runtime / Tool / live repo
- 不新增外部 crate 依赖（std::process::Command + serde_json 已有）
- 不新增 workspace 依赖
- 不修改 workspace Cargo.toml
- 不做 LSP client
- 不做 diagnostics auto-fix
- 不做 incremental diagnostics
- 不嵌入 Cangjie 编译器逻辑

## 4. Expected Public API

```rust
// crates/cangjie/src/diagnostics/types.rs

#[derive(Debug, Clone, Serialize)]
pub struct CangjieDiagnostic {
    pub file_path: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: String,           // "cjc" | "cjlint"
    pub rule: Option<String>,
    pub start_line: usize,        // 0-based
    pub start_column: usize,      // 0-based
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity { Error, Warning, Note, Suggestion }

// crates/cangjie/src/diagnostics/runner.rs

/// Run cjc diagnostics on a single source file. SDK-absent → Ok(vec![])
pub fn run_cjc_diagnostics(source_file: &Path) -> Vec<CangjieDiagnostic>;

/// Run cjlint diagnostics on a project root. SDK-absent → Ok(vec![])
pub fn run_cjlint_diagnostics(project_root: &Path, source_files: &[PathBuf]) -> Vec<CangjieDiagnostic>;

/// Check if Cangjie SDK tools (cjc, cjlint) are available.
pub fn is_cangjie_sdk_available() -> bool;

// crates/cangjie/src/graph.rs (extend)

/// Build diagnostic nodes + ANNOTATES edges from diagnostics list.
pub fn emit_cangjie_diagnostics(
    diagnostics: &[CangjieDiagnostic],
    symbols_by_file: &BTreeMap<PathBuf, Vec<CangjieSymbol>>,
    project_root: &Path,
) -> (Vec<GraphNode>, Vec<GraphEdge>);
```

## 5. Acceptance Criteria

- [ ] `cargo build` 成功（不启用 feature）
- [ ] `cargo build --features tree-sitter-cangjie` 成功
- [ ] `cargo test` 保持已有测试通过（142+）
- [ ] `cargo test --features tree-sitter-cangjie` 全部测试通过
- [ ] SDK-absent 环境下 graceful degrade 测试通过
- [ ] Diagnostic 类型 serde 序列化正确
- [ ] Diagnostic nodes + ANNOTATES edges 产出正确
- [ ] 测试覆盖：types serialization、runner graceful degrade、graph emission
- [ ] `cargo fmt --check` clean
- [ ] `git diff --check` clean
- [ ] 零新增依赖

## 6. Stop-line

- 不实现 LSP client
- 不实现 diagnostics auto-fix
- 不实现 incremental diagnostics
- 不实现 CLI flag
- 不做 cross-file diagnostics correlation
- 不改 project-model / CLI / GitNexus-RC runtime
- 不新增依赖
- 不在无 SDK 环境下 fail
