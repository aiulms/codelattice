# Phase 2 Slice 1 Execution Card — cangjie crate skeleton + cjpm parser

**日期：** 2026-05-06
**状态：** ✅ 完成
**Closure Review：** `docs/plans/2026-05-06-cangjie-phase2-slice1-closure-review.md`
**父计划：** [Phase 1 Preflight](https://gitcode.com/aiulms/gitnexus-rc) — `docs/language-support/plans/2026-05-06-rust-native-cangjie-migration-phase1-preflight.md`

## 1. Scope

新建 Rust-core `crates/cangjie` crate，实现最小 `cjpm.toml` manifest parser（不依赖 tree-sitter、不接 diagnostics、不接 LSP、不接 GitNexus-RC）。

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `Cargo.toml` | 编辑 | 添加 `crates/cangjie` 到 workspace members |
| `crates/cangjie/Cargo.toml` | 新建 | crate 清单，依赖 `toml`、`serde`、`thiserror` |
| `crates/cangjie/src/lib.rs` | 新建 | crate root，`pub mod manifest` |
| `crates/cangjie/src/manifest.rs` | 新建 | cjpm.toml parser：types + `parse_cjpm_toml()` + `load_cjpm_manifest()` |
| `fixtures/cangjie/cjpm-basic/cjpm.toml` | 新建 | 最小 package fixture |
| `fixtures/cangjie/cjpm-basic/src/main.cj` | 新建 | 最小仓颉源文件 |
| `crates/cangjie/tests/` | 新建 | integration tests |

## 3. Forbidden Write Set

- ❌ 不改 `crates/project-model/` 任何文件
- ❌ 不改 `crates/cli/` 任何文件
- ❌ 不改 GitNexus-RC runtime / adapter / schema
- ❌ 不改 GitNexus-RC-Tool
- ❌ 不改 live repo（cangjie）
- ❌ 不新增 tree-sitter 依赖
- ❌ 不新增 LSP / HTTP / MCP 相关代码
- ❌ 不修改 workspace `[workspace.package]` 或 resolver

## 4. Accepted Test Commands

```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo check
cargo test
git diff --check
```

## 5. Stop-line

- 如果 `cargo test` 失败且无法在 3 次内修复，停止并 handoff
- 如果 tree-sitter / LSP 代码入口被意外创建，停止并回退
- 如果 GitNexus-RC 文件被意外修改，停止并检查

## 6. Expected Public API

```rust
// crates/cangjie/src/manifest.rs

/// Parsed cjpm.toml manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieManifest {
    pub package: Option<CangjiePackage>,
    pub workspace: Option<CangjieWorkspace>,
    pub dependencies: Vec<CangjieDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjiePackage {
    pub name: String,
    pub version: Option<String>,
    pub src_dir: String,        // default "src"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieWorkspace {
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieDependency {
    pub name: String,
    pub path: Option<String>,    // path-based dep
    pub version: Option<String>, // version constraint
    pub git: Option<String>,     // git URL
}

/// Parse cjpm.toml from a string.
pub fn parse_cjpm_toml(source: &str) -> Result<CangjieManifest, CangjieManifestError>;

/// Load and parse cjpm.toml from a file path.
pub fn load_cjpm_manifest(path: &Path) -> Result<CangjieManifest, CangjieManifestError>;

#[derive(Debug, thiserror::Error)]
pub enum CangjieManifestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Missing package name")]
    MissingPackageName,
}
```

## 7. Fixture Naming

```
fixtures/cangjie/cjpm-basic/
  cjpm.toml          # [package] name="basic", version="0.1.0", src-dir="src"
  src/
    main.cj          # minimal: main() { println("hello") }
```

Follow-on fixtures (future slices):
- `fixtures/cangjie/cjpm-path-deps/` — path-based dependencies
- `fixtures/cangjie/cjpm-workspace/` — workspace with members

## 8. Dependency Strategy

使用已有 `toml = "0.8"` crate（workspace 中 `project-model` 已依赖），不新增第三方依赖。
`serde` + `serde_json` + `thiserror` 同理复用 workspace 已有版本。

## 9. Closure Requirements

- [x] `cargo fmt --check` clean
- [x] `cargo check` clean（workspace 全体）
- [x] `cargo test` 全部通过（含新增 cangjie tests）
- [x] `git diff --check` clean
- [x] 不引入 `#[allow(...)]` 抑制，除非 closure review 明确说明
- [x] 更新 Rust-core `docs/plans/README.md`
- [ ] 更新 GitNexus-RC `TASK_TRACKER.md` / `plans/README.md` / `RISK_LEDGER.md`
- [ ] Commit + push 两个 repo

## 10. Implementation Notes

- cjpm.toml 使用标准 TOML 格式，`toml` crate 可直接反序列化
- 使用 `#[serde(default)]` 处理可选字段和默认值（如 `src_dir` 默认 `"src"`）
- 不手写 TOML parser；不复制 TS 侧 ~200 行手写 parser 逻辑
- error 类型使用 `thiserror::Error` derive，与 project-model 风格一致
- 测试覆盖：basic package / src-dir default / path dependency / malformed toml
