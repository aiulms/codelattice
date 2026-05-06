# Phase 2 Slice 11 Preflight — Cangjie Import Resolution

日期：2026-05-06
状态：Preflight（待执行）

## 1. 背景

Slice 10 完成了 same-file reference extraction（USES/ACCESSES/MODIFIES edges），但仅限于同文件内的符号解析。要实现跨文件引用解析，必须先解析 Cangjie import 语句，确定每个 import 指向哪个包/文件。

TS adapter 已有完整的 Cangjie import resolution（`import-resolvers/cangjie.ts` + `cjpm-metadata.ts`），本 preflight 评估将其移植到 Rust-core 的可行性与范围。

## 2. Cangjie Import 语法（从 fixture 验证）

```
// 单符号导入
import demo.math.add

// 分组导入
import demo.math.{add, sub}

// 通配符导入
import demo.math.*

// 别名导入
import demo.math.add as plus

// 包别名
import demo.math as math

// 可见性修饰
public import demo.api.add
protected import demo.api.sub
internal import demo.api.internal

// 外部包（std/core 前缀 — 不解析）
import std.collection.*
```

## 3. 现有基础设施

### Rust-core 已有（可直接复用）

| 组件 | 位置 | 能力 |
|------|------|------|
| cjpm.toml 解析 | `manifest.rs` | `CangjieManifest`, `CangjieDependency` (name/path/version/git), workspace members |
| cjpm.lock 解析 | `manifest.rs` | `CjpmLock`, `CjpmLockEntry` (name/version/source/dependencies) |
| Workspace 解析 | `manifest.rs` | `resolve_workspace_manifest()`, `active_members()` |
| Path dep 解析 | `manifest.rs` | `resolve_path_dependency()` → 绝对路径 |
| Project 模型 | `project.rs` | `CangjieProject` (packages, source_files), `find_project_root()` |
| Graph 输出 | `graph.rs` | `NodeKind`/`EdgeKind` enum, `inspect_cangjie_project()` |
| AST 解析 | `extractors/mod.rs` | `parse_cangjie_source()`, tree-sitter API |
| 参考提取 | `extractors/references.rs` | SameFileIndex 模式（可按名查找符号） |

### TS adapter 参考实现

| 函数 | 行数 | 功能 |
|------|------|------|
| `parseCangjieImportTargets()` | ~6 | 拆分 raw import path → targets |
| `extractCangjiePackageAliases()` | ~15 | 从 AST 提取 package alias |
| `extractCangjieImportMetadata()` | ~20 | 提取 visibility + wildcard 检测 |
| `parseCangjieNamedImportCandidates()` | ~80 | 解析 named import → (packageName, exportedName, localName) |
| `candidatePackageDirs()` | ~60 | 多级候选目录生成 |
| `resolvePackageByName()` | ~15 | 遍历候选目录查找 .cj 文件 |
| `resolveCjpmDependencyDir()` | ~25 | path-based dep → 磁盘目录 |
| `resolveTreeDependencyDir()` | ~20 | 递归查找 workspace subtree |
| `parseCjpmTreeOutput()` | ~40 | cjpm tree 文本输出解析（纯文本，无 SDK 依赖） |
| `runCjpmTree()` | ~35 | spawn cjpm 子进程，30s timeout |
| `findPackageDirByName()` | ~25 | 递归查找 cjpm.toml 匹配包名 |

## 4. 选项评估

### 选项 A：完整 Port（~600 行 Rust）

Port TS adapter 全部功能到 Rust，包括 cjpm tree 子进程。

**范围**：
- AST import 解析 (~150 行)
- Raw import path 解析器 (~100 行)
- 候选目录生成 + 包名解析 (~150 行)
- cjpm tree 子进程 + 输出解析 (~100 行)
- Graph IMPORTS edges + 集成 (~100 行)

**优点**：功能完整，与 TS adapter 对等
**缺点**：
- cjpm tree 子进程增加 subprocess 依赖（但已有 cjc/cjlint 先例）
- cjpm tree 输出格式可能因版本变化而漂移
- 较大 scope（~600 行），bounded slice 边界模糊

### 选项 B：最小同一项目 Import 解析（~300 行 Rust）

只解析同一 project/workspace 内的 import。

**范围**：
- AST import 解析 (~120 行)
- Raw import path 字符串解析 (~100 行)
- 同一 workspace member 解析（复用现有 manifest.rs）(~50 行)
- Graph IMPORTS edges + 集成 (~60 行)

**优点**：最小 scope，零新增 subprocess，复用现有基础设施
**缺点**：不解析外部依赖 import（path dep / tree dep）；cross-package 受限

### 选项 C：混合方案 — 包级解析 + Static Deps（~400 行 Rust）（推荐）

Port 核心 import 解析 + 使用现有 cjpm.toml/lock 静态元数据解析依赖，不 spawn cjpm tree。

**范围**：
- AST import 解析 (~120 行)
- Raw import path 字符串解析器（port TS `parseCangjieImportTargets` / `parseNamedCandidates`）(~100 行)
- 候选目录生成器：workspace members + path deps + lock entries (~100 行)
- cjpm.lock source 字段解析：区分 path source vs git source (~30 行)
- Graph IMPORTS edges + `inspect_cangjie_project()` 集成 (~60 行)
- Fixture + tests (~80 行)

**优点**：
- 覆盖主要用例（workspace member + path dep + lock dep）
- 不增加 subprocess 复杂度
- 复用现有 manifest/lock 基础设施
- Bounded scope，可在一个 slice 内完成

**缺点**：
- 不解析 git-based tree dep（需要 cjpm tree 或网络 clone）
- cjpm.lock 的 source 字段对 git dep 可能不直接可用

## 5. 推荐方案：选项 C

### 5.1 设计概要

```
                    ┌─────────────────────────┐
                    │ extract_cangjie_imports() │  ← 新文件 imports.rs
                    │ AST walk: 找到所有 import │
                    │ statement 节点           │
                    └───────────┬─────────────┘
                                │ Vec<CangjieImport>
                                ▼
                    ┌─────────────────────────┐
                    │ resolve_import_target()   │  ← imports.rs
                    │ 1. same workspace?        │
                    │ 2. path dep? → manifest   │
                    │ 3. lock dep? → cjpm.lock  │
                    │ 4. external? → skip       │
                    └───────────┬─────────────┘
                                │ ResolvedImport
                                ▼
                    ┌─────────────────────────┐
                    │ emit_cangjie_import_edges()│ ← graph.rs
                    │ IMPORTS edge:            │
                    │ SourceFile → Package 或   │
                    │ SourceFile → SourceFile  │
                    └─────────────────────────┘
```

### 5.2 新增 API

```rust
// crates/cangjie/src/extractors/imports.rs

/// 从 AST 解析的 import 语句
pub struct CangjieImport {
    pub raw_path: String,           // "demo.math.{add, sub}"
    pub visibility: ImportVisibility,
    pub is_wildcard: bool,          // raw_path.ends_with(".*")
    pub package_alias: Option<PackageAlias>,
    pub file_path: String,
}

pub struct PackageAlias {
    pub package_name: String,
    pub alias: String,
}

pub struct ImportCandidate {
    pub package_name: String,
    pub exported_name: String,
    pub local_name: String,
}

/// 解析 import 的 raw path → 候选列表
pub fn parse_import_targets(raw: &str) -> Vec<String>;
pub fn parse_named_import_candidates(raw: &str) -> Vec<ImportCandidate>;

/// 从 AST tree 提取所有 import 语句
pub fn extract_cangjie_imports(
    source: &str, file_path: &Path, tree: &Tree
) -> Vec<CangjieImport>;

/// 将 import candidate 解析到本地包目录
pub fn resolve_import_target(
    candidate: &ImportCandidate,
    project: &CangjieProject,
) -> Option<ResolvedImport>;

pub struct ResolvedImport {
    pub target_package_name: String,
    pub target_dir: Option<PathBuf>,  // 解析到的包目录
    pub resolution: ResolutionKind,   // WorkspaceMember / PathDep / LockDep / External
}
```

### 5.3 解析策略（对齐 TS adapter）

**候选目录生成顺序**（`candidatePackageDirs` 逻辑）：
1. Workspace member 匹配：包名 == member.package.name → `<moduleDir>/<srcDir>`
2. 子包匹配：`packageName.startsWith(member.name + ".")` → 点号替换为路径分隔符
3. Fallback：`<moduleDir>/<srcDir>/<packageName.replace('.','/')>`
4. Path dep：dep 有 path 字段 → 解析为绝对路径
5. Lock dep：在 cjpm.lock requires 中查找同名 → 检查 source 是否为本地路径

**外部包过滤**：
- `std.*` / `core.*` 前缀 → 跳过（no local resolution）
- 与 TS adapter `isExternalPackage()` 一致

### 5.4 Edge 设计

新增 `EdgeKind::Imports`：

```
SourceFile ──Imports──▶ SourceFile    （同包：import 目标文件）
SourceFile ──Imports──▶ Package       （跨包：import 目标包）
```

不使用 `Imports` 连接 SourceFile → Symbol，因为 import 是文件级别的声明；符号级别的 USES 边由 reference extraction 负责（未来 cross-file 扩展后）。

### 5.5 集成点

在 `inspect_cangjie_project()` 中：
1. 解析每个源文件的 import 语句 → `Vec<CangjieImport>`
2. 对每个 import 解析 target → `ResolvedImport`
3. 生成 IMPORTS edges
4. 合并到 graph output

### 5.6 测试策略

**Fixture**：新建 `fixtures/cangjie/imports-basic/`
- `cjpm.toml`：定义 package + dependencies
- `src/main.cj`：包含多种 import 语法（single, grouped, wildcard, alias, public）
- `src/lib.cj`：被导入的模块（定义 add, sub 等符号）

**Unit tests**（`imports.rs` 内联）：
- `parse_import_targets_single` — "demo.math.add" → ["demo.math.add"]
- `parse_import_targets_grouped` — "{add, sub}" → ["add", "sub"]
- `parse_import_targets_wildcard` — "demo.math.*" → ["demo.math.*"]
- `parse_named_candidates_simple` — "demo.math.add" → candidate
- `parse_named_candidates_grouped` — "demo.math.{add, sub}" → 2 candidates
- `parse_named_candidates_alias` — "demo.math.add as plus" → alias candidate
- `parse_named_candidates_wildcard` — "demo.math.*" → empty
- `is_external_package_std` — "std.collection" → true
- `is_external_package_core` — "core.lang" → true
- `is_external_package_normal` — "demo.math" → false

**Integration tests**（`tests/import_resolution.rs`）：
- `fixture_parses_cleanly` — 验证 fixture 无 ERROR nodes
- `imports_are_extracted` — 验证 import 语句被提取
- `single_import_resolves` — `import demo.math.add` 解析
- `grouped_import_resolves` — `import demo.math.{add, sub}` 解析
- `wildcard_import_resolves` — `import demo.math.*` 解析
- `public_import_metadata` — visibility 正确提取
- `external_package_skipped` — `import std.collection.*` → no resolution
- `imports_edges_in_graph` — 验证 IMPORTS edges 出现在 graph output
- `package_alias_extracted` — `import demo.math as math` → alias

## 6. Required / Forbidden Write Sets

### Required
- `crates/cangjie/src/extractors/imports.rs`（新建，~300 行）
- `crates/cangjie/src/extractors/mod.rs`（新增 pub mod imports + re-exports）
- `crates/cangjie/src/graph.rs`（新增 Imports EdgeKind + emit_cangjie_import_edges() + integrate into inspect）
- `crates/cangjie/src/lib.rs`（re-export new types）
- `fixtures/cangjie/imports-basic/`（新建 fixture）
- `crates/cangjie/tests/import_resolution.rs`（新建 integration tests）

### Forbidden
- 不改 `manifest.rs` / `project.rs` / `references.rs` / `diagnostics/`
- 不新增依赖（纯 stdlib + tree-sitter API + serde 已有）
- 不 spawn cjpm tree（子进程仅 cjc/cjlint）
- 不改 GitNexus-RC runtime
- 不改 Tool / live repo

## 7. Stop-line

- 需要 spawn cjpm tree 子进程 → stop，评估未来方案
- 需要新增依赖 → stop，写 preflight
- 需要修改 schema/CLI contract → stop
- AST import node types 与预期不符 → stop，调查 grammar 版本
- 测试无法通过 → stop，诊断

## 8. 验收标准

1. `cargo fmt --check` clean
2. `cargo check` pass（without feature）
3. `cargo test` pass（without feature，不引入 import 测试）
4. `cargo test --features tree-sitter-cangjie` pass（含 import 测试）
5. 新增 fixture 被正确解析（无 ERROR nodes）
6. 至少 10 个新 tests（unit + integration）
7. IMPORTS edges 出现在 graph output JSON
8. 零新增依赖
9. GitNexus-RC 仅 docs sync（不改 runtime）

## 9. 已知风险

| 风险 | 缓解 |
|------|------|
| tree-sitter-cangjie import AST node types 不确定 | 先打印 AST 验证，再编码 |
| cjpm.lock source 字段对 tree dep 不包含本地路径 | 只处理 path-based dep，lock 仅用于 entry 匹配 |
| Package name → directory mapping 可能与 TS 不一致 | 对齐 TS `candidatePackageDirs()` 逻辑，用 fixture 验证 |

## 10. 后续 Slice（不在本 scope）

- **Slice 11b**：cjpm tree subprocess integration + git dep resolution
- **Slice 12**：Cross-file reference extraction using import resolution
- **Slice 13**：Named import → symbol binding (USES edges for cross-file refs)

## 11. 决策

**推荐：选项 C（混合方案）**

理由：
- 覆盖主要用例（workspace member + path dep + static lock dep）
- Bounded scope（~400 行 Rust，一个 slice 完成）
- 不增加 subprocess 复杂度
- 对齐 TS adapter 核心逻辑但不过度 port
- 为后续 cross-file reference extraction 提供基础
