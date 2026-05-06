# Phase 2 Slice 2 Closure Review — workspace/dependency metadata

**日期：** 2026-05-06
**Execution Card：** `docs/plans/2026-05-06-cangjie-phase2-slice2-execution-card.md`

## Landed Reality

| 项目 | 计划 | 实际 |
|------|------|------|
| workspace resolver | `resolve_workspace_manifest()` | ✅ 已实现 |
| active members | `active_members()` | ✅ 已实现 |
| path dependency resolver | `resolve_path_dependency()` | ✅ 已实现 |
| cjpm.lock parser | `parse_cjpm_lock()` / `load_cjpm_lock()` | ✅ 已实现 |
| fixture | `fixtures/cangjie/cjpm-workspace/` | ✅ root + pkg1 + pkg2 |
| tests | workspace + lock + dep resolver | ✅ 11 new tests（8 unit + 3 integration） |
| cargo fmt | clean | ✅ clean |
| cargo test | 全 pass | ✅ 116/116 pass |

## Verification

```
cargo fmt --check:  clean ✅
cargo test:        116/116 pass ✅ (105 previous + 11 new)
cargo check:       clean ✅
```

## API Surface (新增)

```rust
pub fn resolve_workspace_manifest(root: &Path) -> Result<WorkspaceManifest, ...>;
pub fn active_members(ws: &CangjieWorkspace) -> &[String];
pub fn resolve_path_dependency(dep: &CangjieDependency, manifest_dir: &Path) -> Option<PathBuf>;
pub fn parse_cjpm_lock(source: &str) -> Result<CjpmLock, ...>;
pub fn load_cjpm_lock(path: &Path) -> Result<CjpmLock, ...>;
```

New types: `WorkspaceManifest`, `CjpmLock`, `CjpmLockEntry`.

## Decisions

1. **Missing member silently skipped** — matching TS behavior: `try { readFileSync } catch { skip }`. If a workspace member's cjpm.toml is missing, the resolver continues with other members.
2. **active_members returns `&[String]` slice** — avoids allocation; callers can collect if needed.
3. **cjpm.lock parsed via `toml` crate** — `[[requires]]` table array deserialized with serde, consistent with manifest parsing strategy.

## Residual Risks

- No risk to existing Rust analysis
- No risk to GitNexus-RC runtime
- Cangjie crate remains purely additive
- Likely future extension: `test-members` filtering（TS 侧已实现，Rust 侧留待后续）

## Next Opening

**Phase 2 Slice 3** — baseline project model output:
- project root detection via cjpm.toml
- list source files under src-dir
- emit minimal project metadata JSON or crate API result
- Another bounded slice, no tree-sitter / diagnostics / GitNexus-RC
