# Phase 2 Slice 3 Closure Review — baseline project model output

**日期：** 2026-05-06
**Execution Card：** `docs/plans/2026-05-06-cangjie-phase2-slice3-execution-card.md`

## Landed Reality

| 项目 | 计划 | 实际 |
|------|------|------|
| `find_project_root()` | 从路径向上查找 cjpm.toml | ✅ 已实现 |
| `list_source_files()` | 递归列出 .cj 文件 | ✅ 已实现（跳过 hidden/target/.cache/.generated） |
| `build_project_model()` | 从 workspace 构建 project model | ✅ 已实现 |
| `CangjieProject` / `CangjiePackageInfo` | 类型定义 | ✅ 已定义 |
| fixture | 复用已有 cjpm-basic + cjpm-workspace | ✅ |
| tests | 7 new unit tests | ✅ |
| cargo fmt | clean | ✅ |
| cargo test | 全 pass | ✅ 123/123 pass |

## Verification

```
cargo fmt --check:  clean ✅
cargo test:        123/123 pass ✅ (116 previous + 7 new)
cargo check:       clean ✅
```

## API Surface (Sl3 新增)

```rust
pub fn find_project_root(start: &Path) -> Option<PathBuf>;
pub fn list_source_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error>;
pub fn build_project_model(root: &Path) -> Result<CangjieProject, CangjieManifestError>;
```

New types: `CangjieProject`, `CangjiePackageInfo`.

## Decisions

1. **`find_project_root` finds nearest cjpm.toml** — if called inside a workspace member, returns that member's directory. Callers that want the workspace root should walk up further if the manifest is [package]-only.
2. **`list_source_files` skips build dirs** — target/, .cache/, .generated/ are excluded; hidden dirs (.git, .claude, etc.) are also skipped.
3. **`build_project_model` bundles root package + workspace members** — works for both single-package and workspace projects.

## Residual Risks

- No risk to existing Rust analysis
- No risk to GitNexus-RC runtime
- Cangjie crate remains purely additive

## Next Opening

**Phase 2 Slice 4** — tree-sitter Cangjie preflight / vendor gate:
- 不直接 vendor 大 parser.c，先写 vendor gate / feasibility doc
- 检查 tree-sitter-cangjie 来源、license、ABI、编译方式
- 设计 feature gate
- 这是需要用户 gate 的大依赖/vendor point
