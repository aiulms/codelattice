# Phase 2 Slice 10 Execution Card — Cangjie Same-file Reference Extraction (USES/ACCESSES/MODIFIES)

**日期**: 2026-05-06
**类型**: execution card
**状态**: 进行中
**前置 preflight**: `docs/plans/2026-05-06-cangjie-phase2-slice10-preflight.md`

## 1. Scope

实现 same-file only AST walk reference extraction，产出 USES/ACCESSES/MODIFIES 三种 graph edges。

## 2. Required Write Set

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/cangjie/src/extractors/references.rs` | **新建** | 核心 reference extraction（~350 行） |
| `crates/cangjie/src/extractors/mod.rs` | **编辑** | 新增 `pub mod references;` |
| `crates/cangjie/src/graph.rs` | **编辑** | 新增 Uses/Accesses/Modifies EdgeKind + edge emitter + 集成到 one-shot |

## 3. Forbidden Write Set

- **不改** GitNexus-RC runtime
- **不改** GitNexus-RC-Tool
- **不改** live cangjie repo
- **不改** project-model crate
- **不改** Cargo.toml（零新增依赖）
- **不改** CangjieSymbol / CangjieSymbolKind（复用现有）
- **不实现** cross-file resolution
- **不实现** import-based resolution
- **不实现** type inference / trait solving
- **不实现** macro expansion

## 4. API Design

```rust
/// Reference kinds extracted from Cangjie AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceKind {
    Uses,      // type annotation reference
    Accesses,  // field read access
    Modifies,  // write/mutation
}

/// A reference extracted from Cangjie source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CangjieReference {
    pub kind: ReferenceKind,
    pub source_id: String,        // Method/Constructor/Function node ID
    pub target_name: String,       // referenced symbol name
    pub target_kinds: Vec<CangjieSymbolKind>,
    pub file_path: String,
    pub confidence: f64,
    pub reason: String,
}

/// Extract references from a Cangjie source file using same-file symbol index.
pub fn extract_cangjie_references(
    source: &str,
    file_path: &Path,
    symbols: &[CangjieSymbol],
) -> Result<Vec<CangjieReference>, CangjieParseError>;
```

## 5. Edge Type Mapping

| ReferenceKind | EdgeKind | Reason code |
|--------------|----------|-------------|
| Uses | `Uses` | `cangjie-type-annotation` |
| Accesses | `Accesses` | `cangjie-field-read` |
| Modifies (simple local) | `Modifies` | `cangjie-modifies-assignment` |
| Modifies (compound local) | `Modifies` | `cangjie-modifies-compound` |
| Modifies (field write) | `Modifies` | `cangjie-modifies-field-write` |
| Modifies (field compound) | `Modifies` | `cangjie-modifies-field-compound` |

## 6. Confidence Strategy

| Scenario | Confidence |
|----------|-----------|
| same-file type annotation, unique match | 0.60 |
| same-file field read, unique match | 0.65 |
| same-file local variable write, unique match | 0.85 |
| same-file field write, unique match | 0.80 |
| ambiguous (multiple matches) | no-edge |
| no match | no-edge |
| builtin type | no-edge |

## 7. Acceptance Criteria

1. `cargo build` 成功（feature 关闭时零新增编译）
2. `cargo build --features tree-sitter-cangjie` 成功
3. `cargo test` 所有现有 tests 零回归
4. `cargo test --features tree-sitter-cangjie` 包含新 reference extraction tests
5. `cargo fmt --check` clean
6. 至少 3 类 fixture/tests：
   - type annotation reference（USES edge）
   - field read access（ACCESSES edge）
   - write/mutation（MODIFIES edge）
7. Builtin types 不产生 USES edge（负向测试）
8. Same-file ambiguous 不产生 edge（负向测试）
9. Same-file no match 不产生 edge（负向测试）

## 8. Stop-line

- Same-file only（不跨文件）
- 不新增 Cargo 依赖
- 不修改 CangjieSymbol / CangjieSymbolKind
- 不修改 project-model crate
- 不修改 diagnostics 模块
- 不修改 GitNexus-RC / Tool / live repo

## 9. Test Fixture

`fixtures/cangjie/references-basic/src/main.cj`：

```cangjie
class Point {
    var x: Float64
    var y: Float64
}

struct Size {
    var width: Int64
    var height: Int64
}

func distance(p: Point): Float64 {
    return p.x * p.y
}

func movePoint(p: Point): Unit {
    p.x = 10.0
    p.y += 5.0
    var local: Int64 = 0
    local = 42
}

func identity(x: Int64): Int64 {
    return x
}

func process(points: Array<Point>): Unit {
}
```

## 10. Test Cases

1. **type_annotation_uses_edge**: Point/Size type annotations → USES
2. **field_read_accesses_edge**: p.x / p.y → ACCESSES
3. **field_write_modifies_edge**: p.x = 10.0 → MODIFIES
4. **compound_write_modifies_edge**: p.y += 5.0 → MODIFIES
5. **local_write_modifies_edge**: local = 42 → MODIFIES
6. **builtin_type_no_uses**: Int64/Float64/Unit 不产生 USES
7. **generic_type_argument_uses**: Array<Point> → Point 产生 USES, Array 不产生
8. **parameter_type_uses**: func distance(p: Point) → Point 产生 USES
9. **return_type_uses**: func identity(x: Int64): Int64 → Int64 不产生（builtin）
10. **ambiguous_no_edge**: 两个同名 class → no-edge
11. **no_match_no_edge**: 引用不存在类型 → no-edge

## 11. Verification Commands

```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo check
cargo test
cargo test --features tree-sitter-cangjie
git diff --check
```
