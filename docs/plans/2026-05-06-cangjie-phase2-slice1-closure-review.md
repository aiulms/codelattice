# Phase 2 Slice 1 Closure Review — cangjie crate skeleton + cjpm parser

**日期：** 2026-05-06
**Execution Card：** `docs/plans/2026-05-06-cangjie-phase2-slice1-execution-card.md`

## Landed Reality

| 项目 | 计划 | 实际 |
|------|------|------|
| `crates/cangjie/` | 新建 | ✅ 新建 |
| workspace members | 添加 `crates/cangjie` | ✅ 已添加 |
| manifest parser | `parse_cjpm_toml()` + `load_cjpm_manifest()` | ✅ 已实现 |
| API types | CangjieManifest / CangjiePackage / CangjieWorkspace / CangjieDependency | ✅ 已定义 |
| TOML 策略 | 使用已有 `toml` crate | ✅ 零新增依赖 |
| fixture | `fixtures/cangjie/cjpm-basic/` | ✅ cjpm.toml + src/main.cj |
| tests | basic/pkg/src-dir/path dep/malformed | ✅ 15 tests（13 unit + 2 integration） |
| cargo fmt | clean | ✅ clean |
| cargo check | clean | ✅ clean |
| cargo test | 全 pass | ✅ 105/105 pass |

## Verification

```
cargo fmt --check:  clean ✅
cargo test:        105/105 pass ✅ (89 existing + 16 new cangjie)
cargo check:       clean ✅
git diff --check:  clean ✅
```

## API Surface

```rust
pub fn parse_cjpm_toml(source: &str) -> Result<CangjieManifest, CangjieManifestError>;
pub fn load_cjpm_manifest(path: &Path) -> Result<CangjieManifest, CangjieManifestError>;
```

Types: `CangjieManifest`, `CangjiePackage`, `CangjieWorkspace`, `CangjieDependency`, `CangjieManifestError`.

## Decisions & Deviations

1. **`name` field as `Option<String>`** — serde requires all fields to succeed for struct deserialization. Making `name` optional allows graceful degradation when a `[package]` section has no name, treating it as absent (matching TS behavior where `if (!pkg.name) pkg = undefined`).
2. **`MissingPackageName` variant preserved** — kept in error enum for future stricter validation modes, though not currently emitted.
3. **Dependencies via `toml::Value` + `serde(untagged)`** — handles both simple string (`dep = "1.0"`) and inline table (`dep = { path = "..." }`) formats without hand-rolling a TOML parser.

## Residual Risks

- No risk to existing Rust analysis (calls.rs, imports.rs, graph.rs untouched)
- No risk to GitNexus-RC runtime
- No risk to Tool
- Cangjie crate is purely additive

## Next Opening

**Phase 2 Slice 2** — workspace/dependency metadata:
- Recursive workspace member cjpm.toml parsing
- build-members / test-members filtering
- cjpm.lock minimal parser or placeholder preflight
- path dependency resolution helper
- Fixture: `fixtures/cangjie/cjpm-workspace/` + `fixtures/cangjie/cjpm-path-deps/`

Stop-line remains unchanged. Tree-sitter gate is deferred to Slice 4.
