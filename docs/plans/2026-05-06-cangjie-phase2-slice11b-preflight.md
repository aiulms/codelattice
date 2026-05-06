# Phase 2 Slice 11b Preflight — Cangjie cjpm tree subprocess + external dependency resolution

日期：2026-05-06
状态：preflight（等待 approval）

## Context

Slice 11 实现了 same-project import resolution（workspace member + path dep + lock entry），但存在 known limitation：不解析传递依赖树（transitive dependencies）。

GitNexus-RC TS 侧已有完整 `cjpm tree` subprocess + 解析器 + 集成方案（Round 15, commit `e85291a4`）：
- `parseCjpmTreeOutput()` — 纯文本解析器，解析 `cjpm tree --skip-script` 输出
- `runCjpmTree()` — spawn cjpm 子进程，捕获 tree 输出
- `findPackageDirByName()` / `resolveTreeDependencyDir()` — workspace subtree 递归查找匹配包名

本 Slice 将这些能力移植到 Rust-core cangjie crate。

## Options

| Option | Description | Pro | Con |
|--------|-------------|-----|-----|
| A — Full port | cjpm tree subprocess + parser + dir resolver + integration | 完整补全 Slice 11 已知限制 | 新增 subprocess 路径 |
| B — Parser only | 只实现 `parse_cjpm_tree_output()`，由调用方提供 tree 文本 | 零 subprocess 风险 | 无法 one-shot 集成 |
| C — Defer until LSP | 等 LSP 客户端提供 workspace/package 信息 | 不新增 subprocess | 无 timeline |

**Recommend: Option A** — Full port. Rust-core 已有 subprocess 先例（diagnostics/runner.rs 中的 cjc/cjlint），模式成熟。30s timeout + graceful degrade 策略与 diagnostics runner 一致。

## Scope

### Required Write Set

| 文件 | 操作 |
|------|------|
| `crates/cangjie/src/subprocess/cjpm_tree.rs` | 新建（~200 行）：parse_cjpm_tree_output + run_cjpm_tree + resolve_tree_dependency_dir |
| `crates/cangjie/src/subprocess/mod.rs` | 新建（~15 行）：pub mod cjpm_tree + re-exports |
| `crates/cangjie/src/extractors/imports.rs` | 修改（~80 行）：candidate_package_dirs 新增 tree dep 候选维度; ResolutionKind 新增 TreeDependency |
| `crates/cangjie/src/lib.rs` | 新增 pub mod subprocess + re-export |
| `crates/cangjie/tests/cjpm_tree.rs` | 新建（~8 integration tests） |
| `fixtures/cangjie/imports-basic/` | 扩展 cjpm.toml 加入 dummy path dependency（模拟 tree dep 场景） |

### Forbidden Write Set

- 不改 graph.rs / project.rs / manifest.rs / references.rs / diagnostics/
- 不新增依赖（纯 stdlib + serde 已有）
- 不修改 `inspect_cangjie_project()` 签名
- 不改 GitNexus-RC runtime / Tool / live repo
- 不解析 git-based / registry-based dependency
- 不做 cjpm tree 输出格式的向前兼容保证

### Stop-line

1. 不实现 cjpm tree 以外的 cjpm 子命令
2. 不解析 git dependency 远端 URL
3. 不 clone / fetch 远程仓库
4. 不做 dependency version resolution / conflict detection
5. 子树搜索深度上限保持 MAX_TREE_DEP_SEARCH_DEPTH = 3（对齐 TS）
6. 不修改 inspect_cangjie_project() 签名
7. 不新增 crate 依赖

## API Design

### New Types

```rust
/// A node in the cjpm tree dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CjpmTreeNode {
    pub name: String,
    pub children: Vec<CjpmTreeNode>,
}
```

### New Functions (in `subprocess::cjpm_tree`)

```rust
/// Parse `cjpm tree --skip-script` text output into structured dependency tree.
/// Pure text parser — zero SDK dependency.
pub fn parse_cjpm_tree_output(output: &str) -> Vec<CjpmTreeNode>

/// Run `cjpm tree --skip-script` in the given repo root directory.
/// Returns empty Vec on SDK absent / timeout / non-zero exit.
pub fn run_cjpm_tree(repo_root: &Path) -> Vec<CjpmTreeNode>

/// Check whether cjpm is available via CANGJIE_HOME / CANGJIE_SDK_HOME / PATH.
pub fn is_cjpm_available() -> bool

/// Recursively find a package directory by [package].name in a workspace subtree.
/// Max depth: 3 (aligned with TS MAX_TREE_DEP_SEARCH_DEPTH).
pub fn find_package_dir_by_name(target_name: &str, start_dir: &Path, depth: u32) -> Option<PathBuf>

/// Resolve a tree dependency package name to its src directory on disk.
/// Searches workspace member subtrees for matching cjpm.toml [package].name.
pub fn resolve_tree_dependency_dir(package_name: &str, workspace_roots: &[PathBuf]) -> Option<PathBuf>
```

### ResolutionKind Extension

```rust
pub enum ResolutionKind {
    WorkspaceMember,
    PathDependency,
    LockEntry,
    TreeDependency,  // NEW
    External,
}
```

### Integration Point

在 `candidate_package_dirs()` 中新增第四级 fallback（lock entry 之后、external 之前）：

```
workspace member → path dependency → lock entry → tree dependency → external
```

TS 侧逻辑（`cangjie.ts:279-283`）：
```typescript
if (cangjieConfig && cangjieConfig.packages.length > 0) {
    const wsRoots = cangjieConfig.packages.map(p => p.moduleDir);
    const treeDepDir = resolveTreeDependencyDir(packageName, wsRoots);
    if (treeDepDir) add(treeDepDir);
}
```

Rust 侧 equivalent：在 for 循环末尾，收集 `project.packages[].module_dir` 作为 workspace roots，调用 `resolve_tree_dependency_dir()`。

## cjpm tree Subprocess Strategy

复用 diagnostics/runner.rs 的成熟模式：

1. **Tool discovery**: 复用 `resolve_cangjie_tool("cjpm", "bin")`（diagnostics/runner.rs 已有，提取为 pub）
2. **Spawn env**: 复用 `build_cangjie_spawn_env()`（已 pub）
3. **Timeout**: 30s（对齐 TS `TREE_TIMEOUT_MS = 30000`）
4. **Graceful degrade**: 任何失败返回空 Vec，不影响 import resolution
5. **No polling loop**: 使用 `wait_timeout`（tree 通常 <5s 完成）

## cjpm tree Output Format

```
|-- root_pkg
    └── dep1
        └── subdep
    └── dep2
|-- root_pkg2
    └── dep3
```

- `|--` 标记根节点（ASCII 连字符）
- `└──` 标记子节点（Unicode 框线）
- 每级缩进 4 空格
- 每行格式：`[indent][prefix] package_name`

## Test Plan

### Unit tests（~15）

**parse_cjpm_tree_output（8）：**
- single root, no children
- root with one child
- root with multiple children
- nested children (depth 2)
- multiple roots
- empty output
- lines without tree markers
- real cjpm tree output snapshot

**find_package_dir_by_name（4）：**
- package found at depth 1
- package not found (returns None)
- depth cap exceeded (returns None)
- package name mismatch (returns None)

**resolve_tree_dependency_dir（3）：**
- resolves from workspace roots
- returns None for unknown package
- cache hit (second call returns same result)

### Integration tests（~8）

- cjpm tree subprocess smoke（SDK present 时；absent 时 skip）
- import resolution uses tree dependency fallback
- IMPORTS graph edge emitted for tree-resolved dependency
- candidate_package_dirs includes tree dep candidate
- ResolutionKind::TreeDependency assigned correctly
- graceful degrade when cjpm absent
- deterministic output
- full inspect_cangjie_project includes tree-resolved edges

## Acceptance Criteria

- [ ] cargo fmt --check clean
- [ ] cargo check pass（without feature）
- [ ] cargo test pass（all existing）
- [ ] cargo test --features tree-sitter-cangjie pass（all new + existing）
- [ ] 零新增依赖
- [ ] 不改 graph.rs 签名
- [ ] SDK absent 时 graceful degrade（空 tree，不影响现有行为）
- [ ] IMPORTS edges 数量不低于 Slice 11 baseline（tree dep 只能增加不能减少）

## Risk Assessment

| 风险 | 等级 | 缓解 |
|------|------|------|
| cjpm tree 输出格式变化 | LOW | 纯文本解析器，格式稳定（已验证 TS 侧）；格式变化时 graceful degrade |
| Subprocess spawn 失败 | LOW | 复用 diagnostics runner 模式，已有先例 |
| 子树搜索递归深度 | LOW | MAX_DEPTH = 3，对齐 TS |
| 模块级缓存 stale | LOW | 每次调用 run_cjpm_tree 重建，不跨调用缓存 |
| Unicode 框线跨平台差异 | LOW | `|--` + `└──` 同时支持，TS 已验证 |

## Verified Prerequisites

- [x] diagnostics/runner.rs 已有 `resolve_cangjie_tool()` 和 `build_cangjie_spawn_env()`（可复用）
- [x] `candidate_package_dirs()` 已有 workspace member / path dep / lock entry 三级 fallback
- [x] TS 侧全套逻辑已验证（Round 15, commit `e85291a4`, 67/67 tests pass）
- [x] cjpm 在本地 SDK 可用（source envsetup.sh 后 `which cjpm` 可找到）
- [x] 不触发 Schema / graph 变更
- [x] 不新增 crate 依赖
