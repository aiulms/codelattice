# Phase 2 Slice 7 Preflight — Cangjie graph output / project-model 集成

**日期：** 2026-05-06
**状态：** Preflight（待用户 gate）
**前置：** Slice 6（AST symbol extraction）✅

## 0. 问题定义

Slice 6 已完成 Cangjie AST 符号提取（`CangjieSymbol` + `CangjieSymbolKind`，7 种类型）。现在需要决定：如何将这些符号产出为结构化 output（JSON/graph），使其可被下游消费。

核心问题：**cangjie crate 和 project-model crate 目前是两个独立宇宙。**

| 维度 | project-model | cangjie |
|------|-------------|---------|
| 符号类型 | `Symbol`（22 字段，12 种 SymbolKind） | `CangjieSymbol`（4 字段，7 种 CangjieSymbolKind） |
| 项目模型 | `ProjectModelOutput`（Rust-specific） | `CangjieProject`（cjpm-specific） |
| 清单格式 | `Cargo.toml`（manifest.rs 硬编码） | `cjpm.toml`（manifest.rs 自实现） |
| 源文件 | `.rs`（source.rs 硬编码） | `.cj`（project.rs 自实现） |
| 提取器 | `ItemExtractor` trait（3 个 impl，全部 Rust） | `extract_cangjie_symbols()`（独立函数） |
| 图输出 | `graph.rs`（emit_graph，语言无关） | 无 |

## 1. 集成方案分析

### 方案 A：实现 ItemExtractor trait（project-model 集成）

**做法：** 在 project-model crate 中新增 `CangjieItemExtractor` 实现 `ItemExtractor` trait。

**需要做的：**
1. 扩展 `SymbolKind` 枚举，新增 `Class`、`Interface`（当前 12 种全为 Rust 特定）
2. `CangjieSymbol` → `Symbol` 映射（4 字段 → 22 字段，大量字段为 None/空）
3. 在 cangjie crate 添加对 project-model 的依赖（或反之）
4. 扩展 `create_best_extractor()` 工厂或新增 Cangjie 调度逻辑

**优点：**
- 复用现有 graph emitter（`emit_graph()` 语言无关）
- 统一输出格式（`GraphOutput` JSON schema）
- 长期架构一致性

**缺点：**
- **必须修改 project-model crate**（`SymbolKind` 枚举、可能的 `Symbol` 字段）
- `Symbol` 的 22 个字段中大部分对 Cangjie 无意义（`is_unsafe`、`is_const_fn`、`generic_params`、`impl_details` 等）
- 引入跨 crate 循环依赖风险（cangjie → project-model，project-model ← cangjie）
- 将 Rust 语义泄漏到 Cangjie 符号表示中
- **与 Slice 1-6 stop-line "不改 project-model crate" 冲突**

### 方案 B：Cangjie 独立 graph output（cangjie-native）

**做法：** 在 cangjie crate 内新增 `graph.rs`，直接从 `CangjieProject` + `Vec<CangjieSymbol>` 产出图结构。

**需要做的：**
1. 在 cangjie crate 中定义 Cangjie-specific graph node/edge 类型（或复用 project-model 的 graph 类型）
2. 实现 `emit_cangjie_graph()` 函数
3. 决定是否依赖 project-model 的 graph 类型还是独立定义

**子选项 B1：依赖 project-model 的 graph 类型**
- cangjie crate 添加 `project-model` 依赖（仅 graph 类型）
- 复用 `GraphOutput`、`GraphNode`、`GraphEdge` 等类型
- 不改 project-model 的 item/manifest/source/output 逻辑

**子选项 B2：cangjie 完全独立 graph 类型**
- 在 cangjie crate 内定义自己的 graph 类型
- 与 project-model graph schema 保持结构兼容（相同的 JSON 字段名）
- 零跨 crate 依赖

**优点（B 通用）：**
- **不改 project-model crate**（关键 stop-line 要求）
- cangjie crate 保持自包含
- Cangjie 符号语义不被 Rust 符号模型污染
- 后续如果要统一，只是类型对齐问题

**缺点（B 通用）：**
- 两个 crate 各自有 graph 输出逻辑（代码重复）
- 下游 consumer 需要处理两种 graph 来源
- 长期可能产生 schema 漂移

**优点（B1 vs B2）：**
- 复用现有类型定义，JSON schema 自动一致
- 减少重复代码

**缺点（B1 vs B2）：**
- cangjie 新增对 project-model 的依赖
- project-model 的 graph 类型变更会影响 cangjie

### 方案 C：LanguageAdapter trait（架构重构）

**做法：** 设计和实现 `LanguageAdapter` trait（已在 `docs/architecture/language-adapter-contract.md` 骨架中规划）。

**需要做的：**
1. 设计 `LanguageAdapter` trait（scan/resolve/diagnose/graph 方法）
2. 重构 project-model 为 trait-based 架构
3. 实现 `RustLanguageAdapter` 和 `CangjieLanguageAdapter`
4. 统一 CLI/output 入口

**优点：**
- 正确的长期架构
- 新语言加入成本低
- 消除 project-model 中的 Rust 硬编码

**缺点：**
- **工程量远超一个 bounded slice**（需要重构整个 project-model）
- 需要修改 project-model 几乎所有模块
- 当前只有 2 种语言（Rust + Cangjie），trait 抽象可能过早
- **不在 Phase 2 scope 内**

## 2. 推荐方案

**推荐：方案 B2（cangjie 独立 graph output，不依赖 project-model）**

理由：
1. **遵守 stop-line**：不改 project-model crate
2. **bounded slice**：只新增一个 `graph.rs` 模块（预计 200-400 行）
3. **零新依赖**：不引入跨 crate 依赖
4. **后续可收束**：B2 产出的 JSON schema 与 project-model 的 `GraphOutput` 结构兼容，未来如果实现 LanguageAdapter trait，只需对齐类型
5. **独立性好**：cangjie crate 真正自包含，可独立测试、独立演进

方案 B1 也被接受（依赖 project-model 的 graph 类型），但新增跨 crate 依赖。

方案 A 和 C 超出了当前 slice 的合理范围，应在 LanguageAdapter trait 设计阶段（未来 Phase 3）再评估。

## 3. 推荐方案详细设计

### 3.1 Graph 节点类型

对齐 project-model 的 8 种节点类型，Cangjie graph 使用子集：

| 节点类型 | Cangjie 来源 | 说明 |
|---------|-------------|------|
| Repository | 固定 "cangjie-repo" | 顶层容器 |
| Package | `CangjiePackageInfo.name` | 每个 cjpm package |
| SourceFile | `CangjieProject.source_files` | 每个 .cj 文件 |
| Symbol | `CangjieSymbol` | 7 种符号类型 |
| Diagnostic | （空，Slice 7 不做 diagnostics） | 预留 |

### 3.2 Graph 边类型

| 边类型 | Cangjie 语义 |
|--------|------------|
| CONTAINS_PACKAGE | Repository → Package |
| OWNS_SOURCE | Package → SourceFile |
| DEFINES | SourceFile → Symbol |

### 3.3 符号类型映射

| CangjieSymbolKind | Graph SymbolKind |
|-------------------|-----------------|
| Function | Function |
| Class | Class |
| Struct | Struct |
| Enum | Enum |
| Interface | Interface |
| TypeAlias | TypeAlias |
| Macro | Macro |

注意：`Class` 和 `Interface` 在 project-model 的 `SymbolKind` 中不存在（Rust 无这些概念），但在 Cangjie graph 中是合法的。下游 consumer 需能处理这些类型。

### 3.4 公共 API

```rust
// crates/cangjie/src/graph.rs (新建)

/// Cangjie graph node types (subset of project-model graph schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CangjieGraphNodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
}

/// A node in the Cangjie graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieGraphNode {
    pub id: String,
    pub kind: CangjieGraphNodeKind,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Cangjie graph edge types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CangjieGraphEdgeKind {
    ContainsPackage,
    OwnsSource,
    Defines,
}

/// An edge in the Cangjie graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieGraphEdge {
    pub kind: CangjieGraphEdgeKind,
    pub source_id: String,
    pub target_id: String,
}

/// Top-level Cangjie graph output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CangjieGraphOutput {
    pub nodes: Vec<CangjieGraphNode>,
    pub edges: Vec<CangjieGraphEdge>,
}

/// Build graph output from project model and symbol extraction results.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn emit_cangjie_graph(
    project: &CangjieProject,
    symbols_by_file: &HashMap<PathBuf, Vec<CangjieSymbol>>,
) -> CangjieGraphOutput;
```

### 3.5 JSON schema 兼容性

产出的 JSON 结构与 project-model `GraphOutput` 保持兼容：
- `nodes[].id` — 与 project-model 相同格式（`file:path:Symbol:name`）
- `edges[].source_id` / `edges[].target_id` — 引用 node id
- 节点/边按确定性顺序排列（BTreeMap 排序）

### 3.6 入口集成

```rust
// crates/cangjie/src/lib.rs 或 project.rs 新增

/// Build complete Cangjie project model with symbol extraction and graph output.
#[cfg(feature = "tree-sitter-cangjie")]
pub fn inspect_cangjie_project(root: &Path) -> Result<CangjieGraphOutput, ...> {
    let project = build_project_model(root)?;
    let mut symbols_by_file = HashMap::new();
    for file in &project.source_files {
        let source = std::fs::read_to_string(file)?;
        let symbols = extract_cangjie_symbols(&source)?;
        symbols_by_file.insert(file.clone(), symbols);
    }
    Ok(emit_cangjie_graph(&project, &symbols_by_file))
}
```

## 4. Forbidden（不可协商）

- 不改 project-model crate（`SymbolKind`、`ItemExtractor`、`manifest.rs`、`source.rs`、`output.rs`、`graph.rs`）
- 不改 CLI crate
- 不改 Rust analysis
- 不改 GitNexus-RC runtime / Tool / live repo
- 不新增 workspace 依赖（cangjie crate 不添加对 project-model 的依赖）
- 不新增外部 crate 依赖
- 不实现 diagnostics 节点/边
- 不实现 CALLS / IMPORTS / EXTENDS / IMPLEMENTS 边（无调用图/导入/继承数据）
- 不实现嵌套符号（方法/属性/内部类）
- 不修改 workspace Cargo.toml
- 不设计 LanguageAdapter trait（留待 Phase 3）

## 5. Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/graph.rs` | 新建 | Graph 类型定义 + emit_cangjie_graph() |
| `crates/cangjie/src/lib.rs` | 编辑 | 新增 `pub mod graph`（条件导出） |
| `crates/cangjie/src/project.rs` | 编辑 | 可选：新增 `inspect_cangjie_project()` 入口 |
| `crates/cangjie/tests/graph_smoke.rs` | 新建 | Graph output smoke tests |
| `docs/plans/2026-05-06-cangjie-phase2-slice7-execution-card.md` | 新建 | 执行卡 |

## 6. Acceptance Criteria

- [ ] `cargo build` 成功（不启用 feature）
- [ ] `cargo build --features tree-sitter-cangjie` 成功
- [ ] `cargo test` 保持全部已有测试通过
- [ ] `cargo test --features tree-sitter-cangjie` 新增 graph smoke tests 通过
- [ ] Graph output 包含 Repository/Package/SourceFile/Symbol 节点 + ContainsPackage/OwnsSource/Defines 边
- [ ] 测试覆盖：空项目、单 package 单文件、多文件多符号
- [ ] 使用已有 fixture `fixtures/cangjie/cjpm-basic/` 做 graph smoke
- [ ] `cargo fmt --check` clean
- [ ] `git diff --check` clean
- [ ] 零新增依赖

## 7. Stop-line / Deferred

- 不做 symbol-level CALLS/IMPORTS edges（需要调用图解析）
- 不做 EXTENDS/IMPLEMENTS edges（需要继承分析）
- 不做 Diagnostic nodes（需要 diagnostics runner）
- 不做 project-model ItemExtractor trait 集成
- 不做 LanguageAdapter trait 设计
- 不做 CLI 集成
- 不修改 workspace 结构

## 8. 风险

| 风险 | 缓解 |
|------|------|
| Graph schema 与 project-model 漂移 | Cangjie graph 类型独立定义但 JSON 结构兼容；未来合并时做 schema alignment |
| Class/Interface 是 project-model 中不存在的 SymbolKind | Cangjie graph 使用自己的 SymbolKind 集合；下游 consumer 需处理 |
| 两个 graph 输出路径导致维护负担 | 当前 project-model graph.rs ~762 行，cangjie graph.rs 预计 200-400 行（subset），负担可控 |
| 未来 LanguageAdapter trait 可能需要重做 | B2 方案的独立 graph 类型不影响未来重构；只是多了一个需要迁移的模块 |

## 9. 下一步

如果本 preflight 获批：
1. 写 Slice 7 execution card（冻结 write set + acceptance criteria）
2. 实现 `crates/cangjie/src/graph.rs`
3. 添加 graph smoke tests
4. 闭账 → Slice 8（diagnostics runner 或 LSP client）

如果选方案 B1（依赖 project-model graph 类型）：需额外评估跨 crate 依赖影响。
如果选方案 A/C：需重新评估 scope（远超 bounded slice）。
