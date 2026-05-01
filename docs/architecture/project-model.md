# ProjectModel 模块

> **日期：** 2026-05-01
> **类型：** 模块设计摘要
> **状态：** 草案 from GitNexus-RC

---

## 职责

ProjectModel 模块负责发现和建模 Rust 项目的物理结构：

1. **发现 Cargo manifests** — 找到所有 `Cargo.toml` 文件
2. **分类 packages** — Root vs 子目录 vs workspace member
3. **展开 workspace members** — 显式和 simple glob 模式
4. **确定 targets** — lib、bin、named bin
5. **解析 source ownership** — 哪个 package/target 拥有每个 `.rs` 文件
6. **解析 `crate::` root** — 哪个文件是每个 source file 的 crate root

ProjectModel 输出 **结构化 facts**，不是 graph edges。

---

## 数据模型

### PackageModel

```rust
pub struct PackageModel {
    pub name: String,
    pub manifest_path: PathBuf,
    pub package_root: PathBuf,
    pub targets: Vec<TargetModel>,
    pub feature_names: Vec<String>,
    pub is_workspace_member: bool,
    pub discovery_reason: DiscoveryReason,
}
```

### WorkspaceModel

```rust
pub struct WorkspaceModel {
    pub manifest_path: PathBuf,
    pub workspace_root: PathBuf,
    pub members: Vec<String>,           // manifest 中的 raw members
    pub expanded_members: Vec<PathBuf>, // glob 展开后
}
```

### TargetModel

```rust
pub struct TargetModel {
    pub name: String,
    pub kind: TargetKind,              // Lib, Bin, Test, Bench, Example
    pub crate_root_file: PathBuf,      // 例如 src/lib.rs
    pub source_root_dir: PathBuf,      // 例如 src/
}
```

### SourceOwnership

```rust
pub struct SourceOwnership {
    pub source_path: PathBuf,
    pub package: Option<PackageRef>,
    pub target: Option<TargetRef>,
    pub confidence: Confidence,
    pub reason: OwnershipReason,
}
```

### RootResolution

```rust
pub struct RootResolution {
    pub source_path: PathBuf,
    pub crate_root_file: PathBuf,
    pub target_kind: TargetKind,
    pub confidence: Confidence,
    pub reason: RootReason,
}
```

### Diagnostic

```rust
pub struct Diagnostic {
    pub level: DiagnosticLevel,        // Info, Warning, Error
    pub message: String,
    pub path: Option<PathBuf>,
    pub reason_code: String,
}
```

---

## 不负责什么

ProjectModel **不负责**：

- **Import 解析** — `use` 语句解析到哪个 module
- **Call graph 构建** — 函数调用哪个函数
- **类型推断** — 表达式有什么类型
- **Trait solving** — traits 如何解析
- **宏展开** — 宏展开成什么
- **外部依赖 graph** — workspace 外的 crates
- **Graph 存储** — 写入 LadybugDB 或任何数据库

---

## Confidence 策略

| Tier | Confidence | 典型 Reason |
|------|------------|----------------|
| Exact syntax fact | 0.90-1.00 | `manifest-derived`，`src-bin-not-ignored` |
| Import-resolved | 0.65-0.85 | `crate-root-resolved`，`cargo-workspace-member-resolved` |
| Language heuristic | 0.35-0.70 | （ProjectModel 大多是 exact） |
| Global fallback | ≤ 0.50 | （ProjectModel 中不使用） |

---

## 已知局限

1. 不执行 `cargo metadata` — 仅 manifest-derived 模型
2. 不集成 rust-analyzer — Stop-line
3. 不展开宏 — Stop-line
4. 不求值完整 cfg — `cfgStatus = unknown`
5. 不支持非标准 `[[bin]]` path
6. Tests/examples/benches targets 未完全验证
7. 不支持 complex glob（`**`，多级）
8. 不支持 `package.workspace` override

---

## 来源

- [Rust-core rebuild preflight](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-04-28-rust-core-rebuild-preflight.md)
- [ProjectModel 模块设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-module-design.md)
- [Consolidation handoff review](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-project-model-consolidation-handoff-review.md)
