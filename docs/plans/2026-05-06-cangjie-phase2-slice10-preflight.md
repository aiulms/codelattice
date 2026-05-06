# Phase 2 Slice 10 Preflight — Cangjie AST Reference Extraction (USES/ACCESSES/MODIFIES)

**日期**: 2026-05-06
**状态**: 待用户 gate
**类型**: preflight（docs-only）

## 1. 问题定义

当前 Rust-core Cangjie crate 已具备：
- 符号提取：7 种 top-level symbol（Function/Class/Struct/Enum/Interface/TypeAlias/Macro）
- 图输出：Repository/Package/SourceFile/Symbol/Diagnostic 节点 + ContainsPackage/OwnsSource/Defines/Annotates 边
- 诊断：cjc + cjlint subprocess 集成

**缺失**：符号之间的关系边（references）。当前图中只有 Defines 边（文件→符号定义）和 Annotates 边（诊断→符号），没有：
- **USES**：类型注解引用（如 `var p: Point` 中的 Point → Class/Struct）
- **ACCESSES**：字段读取（如 `obj.field` → Property/Field）
- **MODIFIES**：写入/变更（如 `x = val`、`obj.field += 1` → Variable/Field）

TS adapter 的 `cangjie-reference-extractor.ts`（~730 行）实现了这三种边，但依赖 `ResolutionContext`（4 层符号查找：same-file → named import → import-scoped → global）。Rust-core 尚无 import resolution（计划 Slice 11）。

## 2. TS Adapter 可移植性分析

### 2.1 核心逻辑（可移植）

| 逻辑 | TS 实现 | Rust 移植难度 | 说明 |
|------|---------|--------------|------|
| AST 遍历 | 递归 walk + typeStack/funcStack | **低** | tree-sitter Rust API 有 `Node::children()` / `Node::kind()` / `Node::field_name_for_child()` |
| enclosing type/function 跟踪 | typeStack + funcStack | **低** | 直接翻译为 `Vec<String>` + `Vec<FuncContext>` |
| tree-sitter query（已有 SYMBOL_QUERY） | N/A（TS 用 AST walk） | **低** | 可复用现有 Query 基础设施（symbol.rs 已有 pattern） |
| builtin type 过滤 | BUILTIN_TYPES Set | **低** | 直接翻译为 `phf::Set` 或 `match` |
| field read 检测 | postfixExpression 最后一个 named child 的 fieldAccess | **低** | 同样通过 `node.kind()` 检查 |
| type annotation 提取 | userType → identifier/scoped_identifier | **低** | tree-sitter Rust API 等价 |
| write/mutation 检测 | assignmentExpression LHS 分析 | **低** | 同样通过 `node.kind()` + 操作符文本检测 |
| 1-based → 0-based 归一化 | N/A（TS 用 node ID 字符串） | **低** | 已有模式（diagnostics runner） |

### 2.2 依赖项（需要设计）

| 依赖 | TS 实现 | Rust-core 现状 | 方案 |
|------|---------|---------------|------|
| ResolutionContext | 4 层 lookup（same-file/import/import-scoped/global） | **无** | 见 §3 |
| SymbolIndex | TS `ctx.resolve(name, filePath)` | **无** | 需新建 same-file symbol lookup |
| KnowledgeGraph（节点/边写入） | `graph.addRelationship()` | **有**（CangjieGraphOutput） | 扩展到支持 USES/ACCESSES/MODIFIES |
| Edge ID 生成 | `generateId('USES', ...)` | **有**（symbol_node_id 已有 pattern） | 直接翻译 |
| Source endpoint 验证 | `graph.getNode(id)` check | **无**（Rust-core 用 Vec） | 需构建 source file symbol index 或改为 post-hoc 验证 |

### 2.3 不可移植（当前无法实现）

| 逻辑 | 原因 | 影响 |
|------|------|------|
| import-scoped resolution | 无 import resolution（Slice 11） | cross-file type reference 无法解析 |
| global lookup（lookupClassByName） | 无全局符号索引 | 同 package 跨文件引用无法解析 |
| named import walk（walkBindingChain） | 无 import metadata | 通过 import 引入的类型引用无法解析 |

**结论**：第一刀只能做 **same-file reference extraction**。Cross-file 和 import-based resolution 需要等 Slice 11（import resolution）完成后才能实现。

## 3. 方案评估

### 方案 A：Same-file only AST walk + inline symbol index（推荐）

**设计**：
- 新增 `crates/cangjie/src/extractors/references.rs`（feature-gated）
- AST 递归遍历（同 TS adapter 的 walk 模式），跟踪 enclosing type + function
- 每个文件提取前，先从 `CangjieSymbol` 列表构建 same-file symbol index（`HashMap<name, Vec<&CangjieSymbol>>`）
- 对于 field read / type annotation / write target，只在 same-file index 中查找
- 唯一匹配 → emit edge（confidence 降低一档，标记 `same-file-only`）
- Ambiguous / no match → no-edge

**优点**：
- 零新增依赖
- 不依赖 import resolution
- 与现有 symbol extraction 共享 tree-sitter parse tree（一次 parse，两次使用）
- TS adapter 的 AST walk 逻辑可逐函数翻译

**缺点**：
- 只有 same-file 覆盖（TS adapter same-file 占 ~40% references）
- 后续 Slice 11 完成后需要扩展 resolution 层

**API 设计**：
```rust
pub struct CangjieReference {
    pub kind: ReferenceKind,  // Uses, Accesses, Modifies
    pub source_id: String,     // Method/Constructor/Function node ID
    pub target_name: String,   // 被引用的符号名
    pub target_kinds: Vec<CangjieSymbolKind>,  // 期望的 target 类型
    pub file_path: String,
    pub confidence: f64,
    pub reason: String,
}

pub enum ReferenceKind { Uses, Accesses, Modifies }

pub fn extract_cangjie_references(
    source: &str,
    file_path: &Path,
    symbols: &[CangjieSymbol],
) -> Result<Vec<CangjieReference>, CangjieParseError>;
```

### 方案 B：Tree-sitter query-based extraction

**设计**：用 tree-sitter Query（类似 SYMBOL_QUERY）匹配 fieldAccess/assignmentExpression/userType 等模式

**优点**：声明式，query 模式可读性高

**缺点**：
- query 无法表达 "enclosing function context"（需要知道 field read 发生在哪个 method 内）
- query 无法表达 "skip builtin types" 的条件逻辑
- assignmentExpression 需要文本级操作符检测（= vs +=），query 做不到
- 最终仍需 AST walk 补充上下文信息

**结论**：不推荐。Query 适合 flat pattern 提取（如 symbol），不适合需要上下文跟踪的语义提取。

### 方案 C：跳过 reference extraction，等 import resolution 完成后一起做

**优点**：避免 same-file only 的中间产物

**缺点**：延迟 Slice 10 到 Slice 11 之后，增加 Slice 11 复杂度（import resolution + reference extraction 一起实现会超过 bounded slice 限制）

**结论**：不推荐。same-file reference extraction 本身有价值（即使没有 import resolution），且可以作为 Slice 11 的基础设施。

## 4. 推荐方案 A 详细设计

### 4.1 Write set（Rust-core only）

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/extractors/references.rs` | **新建** | 核心 reference extraction（~350 行） |
| `crates/cangjie/src/extractors/mod.rs` | **编辑** | 新增 `pub mod references;` |
| `crates/cangjie/src/graph.rs` | **编辑** | 新增 Uses/Accesses/Modifies EdgeKind + emit edges |

### 4.2 Forbidden write set

- **不改** GitNexus-RC runtime
- **不改** GitNexus-RC-Tool
- **不改** live cangjie repo
- **不改** project-model crate
- **不改** Cargo.toml（零新增依赖）
- **不实现** cross-file resolution
- **不实现** import-based resolution
- **不实现** type inference / trait solving
- **不实现** macro expansion

### 4.3 Edge type 设计

| EdgeKind | 语义 | 触发条件 | Reason code |
|----------|------|---------|-------------|
| `Uses` | 类型注解引用 | 变量声明/参数/返回类型/generic arg 中的 userType | `cangjie-type-annotation` |
| `Accesses` | 字段读取 | `obj.field`（不带 callSuffix 的 fieldAccess） | `cangjie-field-read` |
| `Modifies` | 写入/变更 | `x = val`、`x += val`、`obj.field = val`、`obj.field += val` | `cangjie-modifies-assignment` / `cangjie-modifies-compound` / `cangjie-modifies-field-write` / `cangjie-modifies-field-compound` |

### 4.4 Confidence 策略

| 场景 | Confidence | 说明 |
|------|-----------|------|
| same-file type annotation，唯一匹配 | 0.60 | 文件内查找，无 import 验证 |
| same-file field read，唯一匹配 | 0.65 | 同上 |
| same-file local variable write，唯一匹配 | 0.85 | AST 中左侧变量直接可见 |
| same-file field write，唯一匹配 | 0.80 | target 是 Property/Field，有额外不确定性 |
| ambiguous（多匹配） | no-edge | 不产生 fake edge |
| no match | no-edge | 不产生 fake edge |
| builtin type | no-edge | Int64/String/Bool 等不产生 USES |

注意：这些 confidence 比 TS adapter 低 0.05（因为缺少 import verification），后续 Slice 11 完成后可恢复。

### 4.5 Reference → Edge 映射

Reference extraction 产出的 `CangjieReference` 需要映射到 graph edges。当前 graph.rs 已有 `EdgeKind::Defines` 和 `EdgeKind::Annotates`。需要新增：

```rust
pub enum EdgeKind {
    // ... existing ...
    Uses,       // Reference → Symbol（type annotation）
    Accesses,   // Reference → Symbol（field read）
    Modifies,   // Reference → Symbol（write/mutation）
}
```

Edge 的 source 是 enclosing Method/Constructor/Function 的 node ID；target 是被引用 symbol 的 node ID。

### 4.6 Same-file symbol index

每个文件提取 references 前，从该文件的 `Vec<CangjieSymbol>` 构建：

```rust
struct SameFileIndex {
    by_name: HashMap<String, Vec<CangjieSymbol>>,
}
```

查找时：`by_name.get(name)` → 按 `target_kinds` 过滤 → 唯一匹配则返回，否则 None。

### 4.7 AST 遍历模式

直接翻译 TS adapter 的 `extractReferences()` 函数：
- `walk(node)` 递归遍历
- `typeStack: Vec<String>` 跟踪 enclosing type（class/struct/interface/enum）
- `funcStack: Vec<FuncContext>` 跟踪 enclosing function/method/init
- `buildSourceId()` 根据上下文构建 Method/Constructor/Function node ID
- 在每个 `postfixExpression` / `variableDeclaration` / `parameter` / `returnType` / `typeArguments` / `assignmentExpression` 节点提取 references

### 4.8 与现有 one-shot 集成

`inspect_cangjie_project()` 已经串联了 symbol extraction → graph emit → diagnostics。Reference extraction 应插入在 symbol extraction 之后、graph emit 之前：

```
build project model
  → extract symbols（每个文件）
  → extract references（每个文件，使用同文件 symbols 构建 same-file index）
  → emit graph（含 Defines + Uses + Accesses + Modifies edges）
  → run diagnostics
  → emit diagnostics（Annotates edges）
```

## 5. Acceptance Criteria

1. `cargo build` 成功（feature 关闭时零新增编译）
2. `cargo build --features tree-sitter-cangjie` 成功
3. `cargo test` 所有现有 tests 零回归
4. `cargo test --features tree-sitter-cangjie` 包含新 reference extraction tests
5. `cargo fmt --check` clean
6. 新增至少 3 类 fixture/tests：
   - type annotation reference（USES edge）→ same-file Class/Struct/Enum/Interface
   - field read access（ACCESSES edge）→ same-file Property/Field
   - write/mutation（MODIFIES edge）→ same-file Variable/Field
7. Builtin types（Int64/String/Bool 等）不产生 USES edge（负向测试）
8. Same-file ambiguous（同名多个 symbol）不产生 edge（负向测试）
9. Same-file no match 不产生 edge（负向测试）
10. Source endpoint 完整性守卫：source 必须是真实存在的 Method/Constructor/Function node

## 6. Stop-line

- **不实现** cross-file resolution（需 Slice 11）
- **不实现** import-based resolution（需 Slice 11）
- **不实现** global lookup（需 Slice 11）
- **不新增** Cargo 依赖
- **不修改** CangjieSymbol / CangjieSymbolKind（复用现有）
- **不修改** project-model crate
- **不修改** diagnostics 模块
- **不修改** GitNexus-RC / Tool / live repo
- **不实现** macro-generated symbol resolution
- **不实现** type inference / trait solving
- **不修改** graph schema（EdgeKind 新增是本地 enum 扩展，不影响外部 contract）

## 7. 风险

| 风险 | 等级 | 缓解 |
|------|------|------|
| Same-file only 覆盖率低 | MEDIUM | 文档明确标注 known limitation；Slice 11 完成后扩展 |
| AST walk 遗漏 Cangjie 语法边缘 | LOW | 翻译 TS adapter 已验证的 walk 逻辑（已在 GitNexus-RC 通过 136 tests） |
| Confidence 低于 TS adapter | LOW | 降低 0.05 是诚实的（缺少 import verification），而非 regression |
| Source endpoint 可能 dangling | LOW | 用 same-file symbol index 反向验证 sourceId 对应的 symbol 存在 |
| tree-sitter-cangjie grammar bug 影响 extraction | LOW | 已有已知 bug 列表（RISK_LEDGER §3.3），新 fixtures 必须验证 `!tree_has_error_nodes()` |

## 8. 测试设计

### 8.1 Fixture 文件

新建 `fixtures/cangjie/references-basic/src/main.cj`：

```cangjie
// Type annotation reference → USES edge
class Point {
    var x: Float64
    var y: Float64
}

struct Size {
    var width: Int64
    var height: Int64
}

// Field read access → ACCESSES edge
func distance(p: Point): Float64 {
    return p.x * p.y
}

// Write/mutation → MODIFIES edge
func movePoint(p: Point): Unit {
    p.x = 10.0        // field write → MODIFIES
    p.y += 5.0        // compound field write → MODIFIES
    var local: Int64 = 0
    local = 42        // local variable write → MODIFIES
}

// Builtin types → no USES edge（负向验证）
func identity(x: Int64): Int64 {
    return x
}

// Generic type argument → USES edge
func process(points: Array<Point>): Unit {
    // Array 是 builtin，不产生 USES；Point 应该产生 USES
}
```

### 8.2 测试用例

1. **type_annotation_uses_edge**：Point/Size 类型注解 → USES edges
2. **field_read_accesses_edge**：p.x / p.y → ACCESSES edges
3. **field_write_modifies_edge**：p.x = 10.0 → MODIFIES edge
4. **compound_write_modifies_edge**：p.y += 5.0 → MODIFIES edge
5. **local_write_modifies_edge**：local = 42 → MODIFIES edge
6. **builtin_type_no_uses**：Int64/Float64/Unit 不产生 USES
7. **generic_type_argument_uses**：Array<Point> → Point 产生 USES，Array 不产生
8. **parameter_type_uses**：func distance(p: Point) → Point 产生 USES
9. **return_type_uses**：func identity(x: Int64): Int64 → Int64 不产生（builtin）
10. **ambiguous_no_edge**：两个同名 class 在同一文件 → no-edge
11. **no_match_no_edge**：引用不存在的类型 → no-edge

## 9. 与 TS Adapter 对比

| 维度 | TS Adapter | Rust-core Slice 10（方案 A） |
|------|-----------|---------------------------|
| Resolution 层 | 4 层（same-file/import/import-scoped/global） | 1 层（same-file only） |
| Cross-file | ✅（通过 ctx.resolve） | ❌（待 Slice 11） |
| Import-based | ✅（walkBindingChain） | ❌（待 Slice 11） |
| AST walk 模式 | typeStack + funcStack | 同 |
| Builtin 过滤 | BUILTIN_TYPES Set | 同 |
| Edge 类型 | USES/ACCESSES/MODIFIES | 同 |
| Confidence | 0.60-0.85 | -0.05（无 import verification） |
| 代码量 | ~730 行 | ~350 行（去掉 resolution 层） |
| 新增依赖 | 零（TS 侧同为纯 AST 操作） | 零 |

## 10. 实现顺序

1. 新建 `references.rs` → AST walk + same-file index + extraction 逻辑
2. 扩展 graph.rs → Uses/Accesses/Modifies EdgeKind + edge emitter
3. 集成到 `inspect_cangjie_project()` one-shot
4. 新建 fixture + tests
5. `cargo fmt --check && cargo check && cargo test`
6. Docs sync + commit

## 11. Next steps

- **If approved**: 写 execution card → implement Slice 10（~350 行 Rust）
- **If deferred**: 跳至 Slice 11 preflight（import resolution，需 cjpm tree + lock-based resolution）
- **If alternative**: 方案 C（一起做）= 写 Slice 10+11 合并 preflight
