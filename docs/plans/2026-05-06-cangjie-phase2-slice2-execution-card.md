# Phase 2 Slice 2 Execution Card — workspace/dependency metadata

**日期：** 2026-05-06
**状态：** ✅ 完成
**Closure Review：** `docs/plans/2026-05-06-cangjie-phase2-slice2-closure-review.md`
**前置：** Slice 1（cangjie crate skeleton + cjpm parser）✅

## 1. Scope

在已有 `parse_cjpm_toml()` 基础上扩展：
1. workspace member 递归解析（`resolve_workspace_manifest`）
2. build-members 过滤（`active_members`）
3. cjpm.lock 最小 parser（`parse_cjpm_lock`）
4. path dependency resolution helper

不接 tree-sitter / diagnostics / LSP / GitNexus-RC。

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/manifest.rs` | 编辑 | 新增 workspace resolver + lock parser + path dep helper |
| `crates/cangjie/src/lib.rs` | 编辑 | 导出新 public API |
| `fixtures/cangjie/cjpm-workspace/cjpm.toml` | 新建 | workspace root fixture |
| `fixtures/cangjie/cjpm-workspace/pkg1/cjpm.toml` | 新建 | member 1 |
| `fixtures/cangjie/cjpm-workspace/pkg2/cjpm.toml` | 新建 | member 2 |
| `crates/cangjie/tests/manifest_integration.rs` | 编辑 | 新增 workspace/lock/dep 测试 |

## 3. Expected Public API

```rust
/// Resolve workspace: load root cjpm.toml, recursively parse each member.
pub fn resolve_workspace_manifest(root: &Path) -> Result<WorkspaceManifest, CangjieManifestError>;

pub struct WorkspaceManifest {
    pub root: CangjieManifest,
    pub members: Vec<(String, CangjieManifest)>, // (member_dir, manifest)
}

/// Return active members after build-members filtering.
pub fn active_members(ws: &CangjieWorkspace) -> &[String];

/// Resolve path-based dependency to absolute directory.
pub fn resolve_path_dependency(dep: &CangjieDependency, manifest_dir: &Path) -> Option<PathBuf>;

/// Parse cjpm.lock ([[requires]] entries).
pub fn parse_cjpm_lock(source: &str) -> Result<CjpmLock, CangjieManifestError>;
```

## 4. Fixtures

```
fixtures/cangjie/cjpm-workspace/
  cjpm.toml              # [workspace] members=["pkg1","pkg2"]
  pkg1/cjpm.toml         # [package] name="pkg1"
  pkg2/cjpm.toml         # [package] name="pkg2"
```

## 5. Stop-line

与 Slice 1 相同。不接 tree-sitter / diagnostics / LSP / GitNexus-RC。
