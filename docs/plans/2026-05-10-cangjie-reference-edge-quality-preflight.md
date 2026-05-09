# Cangjie Reference Edge Quality Preflight

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Preflight（bounded quality improvement）
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)、[QUALITY.md](../../QUALITY.md)

---

## 一、目标

固化 Cangjie import/reference/call edge confidence/reason/no-edge 行为，重点是：
1. 显式 import、package alias、wildcard import 的 confidence 差异
2. Ambiguous 场景的 no-edge 行为验证
3. 补齐 alias/wildcard/ambiguous 测试断言
4. 如发现 mismatch，做最小修复

---

## 二、现有 Confidence/Reason 矩阵

来源：`crates/cangjie/src/extractors/references.rs` + `imports.rs`

### 2.1 Same-file Reference Edges

| # | Edge Type | Scenario | Confidence | Reason |
|---|-----------|----------|-----------|--------|
| 1 | Uses | Type annotation (variable/param/return/generic) | 0.60 | cangjie-type-annotation |
| 2 | Uses | Function/constructor call | 0.80 | cangjie-function-call |
| 3 | Accesses | Field read | 0.65 | cangjie-field-read |
| 4 | Modifies | Simple assignment | 0.85 | cangjie-modifies-assignment |
| 5 | Modifies | Compound assignment (+=/-=) | 0.85 | cangjie-modifies-compound |
| 6 | Modifies | Field write | 0.80 | cangjie-modifies-field-write |
| 7 | Modifies | Field compound write | 0.80 | cangjie-modifies-field-compound |

### 2.2 Cross-file Import Resolution

| # | Import Kind | Cross-file Confidence | Reason Pattern |
|---|-------------|----------------------|----------------|
| 1 | ExplicitImport | 0.85 | `{reason} (cross-file via explicit import)` |
| 2 | PackageAlias | 0.80 | `{reason} (cross-file via package alias)` |
| 3 | WildcardImport | 0.70 | `{reason} (cross-file via wildcard import)` |

### 2.3 No-edge Policy

| Scenario | Behavior |
|----------|----------|
| Ambiguous (multiple import matches) | **No edge** — `push_reference` silently returns |
| Builtin type reference | **No edge** — filtered by `is_builtin_type()` |
| No same-file match + no import binding | **No edge** — `push_reference` silently returns |

---

## 三、ImportKind 定义

来源：`crates/cangjie/src/extractors/references.rs:318`

```rust
pub enum ImportKind {
    ExplicitImport,    // import pkg.Func
    WildcardImport,    // import pkg.*
    PackageAlias,      // import pkg as p
}
```

---

## 四、测试覆盖分析

### 4.1 已有测试

| Test File | What It Tests | Has Confidence/Reason Assertions? |
|-----------|--------------|-----------------------------------|
| `reference_extraction.rs` | Same-file Uses (type annotation + function call) | ✅ `assert_eq!(confidence, 0.60)` and `0.80` |
| `cross_file_reference.rs` | Cross-file Uses via explicit import | ✅ `assert_approx!(confidence, 0.85)` and reason contains "cross-file" |
| `alias_reference.rs` | Package alias + wildcard import resolution | ❌ Uses ImportKind but **NO confidence/reason assertions** |
| `import_resolution.rs` | Import parsing + graph integration | ❌ No confidence/reason assertions |
| `function_call_reference.rs` | Function/constructor call references | Unknown — needs check |
| `constructor_extraction.rs` | Init symbols | N/A (symbol extraction, not edges) |
| `graph_contract.rs` | Known edge triples | ✅ Edge existence but not confidence values |
| `endpoint_integrity.rs` | Dangling edge check | ✅ Structural |
| `multi_project_smoke.rs` | Quality gates | ✅ Duplicate/dangling/deterministic |

### 4.2 关键缺口

1. **alias_reference.rs** — 测试使用 `ImportKind::PackageAlias`、`ImportKind::WildcardImport`、`ImportKind::ExplicitImport`，但**不验证 confidence 值**。PackageAlias 应得 0.80，WildcardImport 应得 0.70，ExplicitImport 应得 0.85。

2. **import_resolution.rs** — 验证 import 解析为正确的文件路径，但不验证引用边的 confidence。

3. **无 wildcard_reference.rs** — 不存在独立的 wildcard import 边界测试。

4. **无 ambiguous no-edge test** — 没有测试验证当多个 import match 同一 symbol 时确实不产生边。

---

## 五、Fixture 覆盖

| Fixture | Scenario | Test File |
|---------|----------|-----------|
| `imports-basic` | Named/grouped/wildcard/alias imports | `graph_contract.rs`, `import_resolution.rs` |
| `references-basic` | Same-file Uses edges | `reference_extraction.rs` |
| `reference-cross-file-basic` | Cross-file Uses + Imports | `cross_file_reference.rs` |
| `reference-function-call-basic` | Function call references | `function_call_reference.rs` |
| `reference-function-call-cross-file` | Cross-file function calls | `function_call_reference.rs` |
| `constructor-basic` | Multi-init + Uses edges | `graph_contract.rs` |
| `constructor-cross-file` | Cross-file Init | `constructor_extraction.rs` |
| `portable-smoke` | All symbol kinds + edges | `graph_contract.rs`, `multi_project_smoke.rs` |

---

## 六、本 Slice 的 Write Set

| 操作 | 文件 | 说明 |
|------|------|------|
| 新增 | `docs/plans/2026-05-10-cangjie-reference-edge-quality-preflight.md` | 本文档 |
| 修改 | `crates/cangjie/tests/alias_reference.rs` | 补齐 confidence/reason 断言 |
| 可能新增 | `crates/cangjie/tests/wildcard_import_confidence.rs` | Wildcard import confidence 专项测试（如 alias_reference 不够） |
| 可能新增 | 小 fixture（如 ambiguous import scenario） | 仅在现有 fixture 不够时 |
| 新增 | `docs/plans/2026-05-10-cangjie-reference-edge-quality-closure.md` | Closure 文档 |

**不修改的文件：**
- `crates/cangjie/src/extractors/references.rs` — confidence 值和 reason 字符串已正确
- `crates/cangjie/src/extractors/imports.rs` — import 解析逻辑不变
- `crates/cangjie/src/graph.rs` — edge 发射逻辑不变

---

## 七、Stop-line 确认

- ❌ No full type inference
- ❌ No interface/trait solving
- ❌ No macro expansion
- ❌ No LSP daemon integration
- ❌ No live repo writes
- ❌ No default tool switch
- ✅ Add confidence/reason assertions to existing tests
- ✅ Verify no-edge behavior for ambiguous imports
- ✅ Document confidence matrix

---

## 八、结论

Cangjie edge confidence/reason 质量合同现状**基本良好**：
- 7 种 same-file edge + 3 种 cross-file confidence 层级定义清晰
- no-edge 策略明确（ambiguous → 不产生边）
- 关键缺口在 `alias_reference.rs` 缺少 confidence/reason 断言

**本 slice 主要产出：** 补齐 alias_reference 测试断言 + 可选的 wildcard/ambiguous 专项测试 + 文档矩阵。confidence/reason 值本身不需要修复。
