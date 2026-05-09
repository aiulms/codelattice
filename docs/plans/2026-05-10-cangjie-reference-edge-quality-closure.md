# Cangjie Reference Edge Quality Closure

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Closure Review
> **关联：** [Preflight](2026-05-10-cangjie-reference-edge-quality-preflight.md)

---

## 一、完成概要

Cangjie import/reference/call edge confidence/reason 质量合同审查完成。**补齐了测试断言，无 behavior change。**

---

## 二、变更文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `docs/plans/2026-05-10-cangjie-reference-edge-quality-preflight.md` | 新增 | Preflight 文档 |
| `crates/cangjie/tests/alias_reference.rs` | 修改 | 新增 4 个 confidence/reason 测试（+152 行） |
| `crates/cangjie/tests/cross_file_import_confidence.rs` | 新增 | 7 个跨文件 import confidence/reason/ambiguous 测试（456 行） |
| `docs/plans/2026-05-10-cangjie-reference-edge-quality-closure.md` | 新增 | 本文档 |

**未修改的文件：**
- `crates/cangjie/src/extractors/references.rs` — confidence 值和 reason 字符串已正确
- `crates/cangjie/src/extractors/imports.rs` — import 解析逻辑不变
- `crates/cangjie/src/graph.rs` — edge 发射逻辑不变

---

## 三、Confidence/Reason 矩阵（未变化）

### Same-file Edges

| Edge | Confidence | Reason |
|------|-----------|--------|
| Type annotation | 0.60 | cangjie-type-annotation |
| Function call | 0.80 | cangjie-function-call |
| Field read | 0.65 | cangjie-field-read |
| Modifies (assignment) | 0.85 | cangjie-modifies-assignment |
| Modifies (compound) | 0.85 | cangjie-modifies-compound |
| Field write | 0.80 | cangjie-modifies-field-write |
| Field compound | 0.80 | cangjie-modifies-field-compound |

### Cross-file Import Resolution

| Import Kind | Confidence | Reason Pattern |
|-------------|-----------|----------------|
| ExplicitImport | 0.85 | {base} (cross-file via explicit import) |
| PackageAlias | 0.80 | {base} (cross-file via package alias) |
| WildcardImport | 0.70 | {base} (cross-file via wildcard import) |

### No-edge Policy

| Scenario | Behavior |
|----------|----------|
| Ambiguous (multiple matches) | No edge (silently dropped) |
| Builtin type | No edge (filtered) |
| No match | No edge (silently dropped) |

---

## 四、新增/修改 Tests

### alias_reference.rs — 4 new tests

| Test | What It Asserts |
|------|----------------|
| `test_package_alias_direct_confidence` | PackageAlias binding → confidence=0.80, reason contains "cross-file via package alias" |
| `test_import_binding_exact_match_priority` | ExplicitImport preferred over WildcardImport → confidence=0.85 |
| `test_wildcard_import_confidence_and_reason` | WildcardImport binding → confidence=0.70, reason contains "cross-file via wildcard import" |
| (existing tests extended) | Confidence/reason assertions added to 3 existing tests |

### cross_file_import_confidence.rs — 7 new tests

| Test | What It Asserts |
|------|----------------|
| `test_explicit_import_confidence_and_reason` | ExplicitImport via fixture → confidence=0.85, reason correct |
| `test_package_alias_confidence_and_reason` | PackageAlias via inline test → confidence=0.80, reason correct |
| `test_wildcard_import_confidence_and_reason` | WildcardImport via inline test → confidence=0.70, reason correct |
| `test_ambiguous_import_produces_no_edge` | Two ExplicitImport matches → no reference edge produced |
| `test_ambiguous_mixed_kinds_produces_no_edge` | ExplicitImport + WildcardImport conflict → no reference edge |
| `test_confidence_ranking_explicit_beats_wildcard` | ExplicitImport preferred when both available |
| `test_fixture_imports_basic_with_import_bindings` | Integration with imports-basic fixture |

---

## 五、Resolution Rate / Graph Stats

**无变化。** 新增测试只验证已有行为，不修改运行时逻辑。

---

## 六、Stop-line

**未触碰。** 不做类型推断、不做 trait solving、不做宏展开。

---

## 七、验证结果

| 验证项 | 结果 |
|--------|------|
| cargo fmt --check | ✅ PASS |
| git diff --check | ✅ PASS |
| alias_reference | 11/11 ✅ (4 new + 7 existing) |
| cross_file_import_confidence | 7/7 ✅ (all new) |
| bridge_roundtrip (no feature) | 13/13 ✅ |
| bridge_roundtrip (with cangjie) | 13/13 ✅ |
| productization_commands | 30/30 ✅ |
| alpha-trial-smoke --rust-only | 5/5 ✅ |
| alpha-trial-smoke --cangjie-only | 5/5 ✅ |
| Tool status | up-to-date ✅ |
| 0 dangling / 0 duplicate | ✅ maintained |

---

## 八、后续 Opening

- 低优先级：为 `function_call_reference.rs` 补齐 confidence 断言（当前只验证存在性）
- 低优先级：创建 Cangjie confidence matrix 文档（对称 Rust 的 `rust-calls-confidence-matrix.md`）
