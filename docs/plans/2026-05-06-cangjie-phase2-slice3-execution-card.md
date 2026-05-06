# Phase 2 Slice 3 Execution Card — baseline project model output

**日期：** 2026-05-06
**状态：** 待执行
**前置：** Slice 2（workspace resolver + lock parser）✅

## 1. Scope

在不接 tree-sitter 的情况下，让 Rust-core 能识别 Cangjie package/project：
1. `find_project_root()` — 从任意路径向上查找 cjpm.toml
2. `list_source_files()` — 递归列出 src-dir 下所有 `.cj` 文件
3. `CangjieProject` / `CangjiePackageInfo` — 最小项目元数据类型
4. `build_project_model()` — 从 workspace manifest 构建 project model

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/lib.rs` | 编辑 | pub mod project |
| `crates/cangjie/src/project.rs` | 新建 | project model types + functions |
| `crates/cangjie/tests/manifest_integration.rs` | 编辑 | 新增 project model tests |

## 3. Forbidden

- 不接 tree-sitter
- 不接 diagnostics
- 不接 LSP
- 不接 GitNexus-RC
- 不改 project-model crate
- 不改 CLI crate

## 4. Expected Public API

```rust
pub fn find_project_root(start: &Path) -> Option<PathBuf>;
pub fn list_source_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error>;
pub fn build_project_model(root: &Path) -> Result<CangjieProject, CangjieManifestError>;
```

## 5. Stop-line

与 Slice 1/2 相同。不接 tree-sitter / diagnostics / LSP / GitNexus-RC。
