# Cangjie Import/Reference Quality Hardening — Closure Review

**Date:** 2026-05-08  
**Status:** Closure Review  
**Type:** Quality Hardening  
**Parent:** Phase 2 autonomous advancement — Cangjie import/reference quality hardening

---

## 问题

在 preflight 调查中发现三个 import/reference 质量问题：

### 1. Confidence 扁平化（CRITICAL）
所有跨文件 import（explicit、wildcard、package alias）统一使用 0.85 confidence，无法区分不同 import 种类的信号质量差异。

### 2. Disambiguation 策略有 bug
`apply_disambiguation_heuristics()` 返回硬编码的 `Some(0)`（始终返回第一个候选），导致即使存在更优候选（如 explicit import），也可能返回错误候选。

### 3. 未使用的 confidence 计算代码
`calculate_wildcard_confidence()`、`detect_symbol_conflicts()` 等 ~100 行 confidence 计算代码虽然计算了 adjusted confidence，但从未存储或使用（flat 0.85 覆盖了一切）。这些 dead code 增加了维护负担。

---

## 修复策略

### W1: ImportKind 枚举 + 差异化 confidence

1. 新增 `ImportKind` 枚举（`ExplicitImport` / `WildcardImport` / `PackageAlias`）
2. `ImportBinding` 新增 `import_kind: ImportKind` 字段
3. `ImportBindingTable::build()` 在创建三种 binding 时设置正确的 `import_kind`
4. `push_reference()` 根据 `import_kind` 使用差异化 confidence：
   - `ExplicitImport` → 0.85（最高，直接命名 import）
   - `PackageAlias` → 0.80（间接，需前缀解析）
   - `WildcardImport` → 0.70（最低，启发式展开）
5. Reason 字符串同样区分：`"cross-file via explicit import"` / `"cross-file via package alias"` / `"cross-file via wildcard import"`

### W2: Disambiguation 重写

`apply_disambiguation_heuristics()` 重写为确定性优先级策略：

1. **Priority 1**: Unique ExplicitImport — 唯一确定性选择
2. **Priority 2**: Unique PackageAlias — 唯一但间接
3. **Priority 3**: 多个 wildcard 或混合无 clear winner → `None`（no-edge）

核心原则："宁可 no-edge，也不要错误高置信度 edge"

### W3: Dead code 清理

移除未使用的代码：
- `SymbolConflict` struct
- `detect_symbol_conflicts()` 方法
- `calculate_wildcard_confidence()` 函数
- `extract_package_from_path()` 函数
- `calculate_specificity_score()` 函数
- `ImportBindingTable::build()` 中的 `symbol_frequency` / `total_symbols` 追踪

### W4: Warning cleanup

- `imports.rs::package_name_from_target()`: 添加 `#[allow(dead_code)]`（Slice 11 遗留，仅 test 中使用）
- `import_resolution.rs`: 移除未使用的 `extract_cangjie_symbols` import
- `import_resolution.rs`: 前缀未使用变量 `file_path` → `_file_path`

### W5: 测试启用

- `test_import_binding_exact_match_priority`: 移除 `#[ignore]` — 验证 ExplicitImport 优先级
- `test_import_binding_no_ambiguous_resolution`: 移除 `#[ignore]` — 验证多 wildcard 冲突 no-edge
- 所有 7 个 `ImportBinding` 构造函数更新为包含 `import_kind` 字段

---

## 实现变更

### `crates/cangjie/src/extractors/references.rs`

1. 新增 `ImportKind` enum（3 variants, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`）
2. `ImportBinding` 新增 `import_kind: ImportKind` 字段
3. `ImportBindingTable::build()`: 移除 symbol frequency 追踪；为 explicit/wildcard/alias binding 设置正确 `import_kind`
4. `apply_disambiguation_heuristics()`: 重写为 ImportKind-based 三级优先级
5. `push_reference()`: 根据 `import_kind` 使用差异化 confidence + reason
6. 移除: `SymbolConflict`, `detect_symbol_conflicts()`, `calculate_wildcard_confidence()`, `extract_package_from_path()`, `calculate_specificity_score()`

### `crates/cangjie/tests/alias_reference.rs`

1. 新增 `ImportKind` import
2. 移除未使用的 `ImportCandidate` import
3. 所有 `ImportBinding` 构造函数添加 `import_kind` 字段（7 处）
4. 启用 2 个 `#[ignore]` 测试

### `crates/cangjie/src/extractors/imports.rs`

1. `package_name_from_target()`: 添加 `#[allow(dead_code)]`

### `crates/cangjie/tests/import_resolution.rs`

1. 移除未使用的 `extract_cangjie_symbols` import
2. 前缀未使用变量 `file_path` → `_file_path`

---

## Confidence 策略矩阵

| Import Kind | Confidence | Reason Suffix | 触发条件 |
|------------|-----------|---------------|---------|
| `ExplicitImport` | 0.85 | `cross-file via explicit import` | 命名 import（simple/grouped/alias） |
| `PackageAlias` | 0.80 | `cross-file via package alias` | `import pkg as p` prefix resolution |
| `WildcardImport` | 0.70 | `cross-file via wildcard import` | `import pkg.*` heuristic expansion |
| Same-file | 0.60–0.85 | `same-file ...` | 同一文件内的 definition → reference |

## Disambiguation 策略

| 场景 | 候选 | 行为 | 结果 |
|------|------|------|------|
| 1 explicit + N wildcard | explicit wins | `Some(explicit_index)` | Uses edge, confidence 0.85 |
| 1 alias + N wildcard | alias wins | `Some(alias_index)` | Uses edge, confidence 0.80 |
| N wildcard only | no unique winner | `None` | No edge |
| Mixed explicit + alias | no unique winner | `None` | No edge |
| Single candidate (any kind) | unique | `Some(0)` | Uses edge, confidence per kind |

---

## 验证结果

- `cargo fmt --check`: ✅ clean
- `cargo check --features tree-sitter-cangjie`: ✅ clean (only pre-existing scanner.c warnings)
- `cargo test --features tree-sitter-cangjie`: ✅ all pass (0 fail, 1 opt-in `#[ignore]`)
- `cargo test --features tree-sitter-cangjie --test alias_reference -- --nocapture`: ✅ 9/9 pass (2 previously `#[ignore]` now enabled)
- `cargo test --features tree-sitter-cangjie --test import_resolution -- --nocapture`: ✅ 10/10 pass
- `cargo test --features tree-sitter-cangjie --test cross_file_reference -- --nocapture`: ✅ 3/3 pass
- `cargo test --features tree-sitter-cangjie --test reference_extraction -- --nocapture`: ✅ 7/7 pass
- `cargo test --features tree-sitter-cangjie --test endpoint_integrity -- --nocapture`: ✅ 12/12 pass
- `cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`: ✅ all 4 targets pass
- `git diff --check`: ✅ clean

### Production smoke (4 targets)

| Target | Duplicate Nodes | Duplicate Edges | Dangling Source | Dangling Target | Deterministic |
|--------|----------------|-----------------|-----------------|-----------------|---------------|
| cjgui (GitNexus-Index) | 0 | 0 | 0 | 0 | true |
| cjgui (cangjie) | 0 | 0 | 0 | 0 | true |
| web_framework | 0 | 0 | 0 | 0 | true |
| json_parser | 0 | 0 | 0 | 0 | true |

---

## 禁止事项遵守

- ✅ 不改 GitNexus-RC
- ✅ 不改 GitNexus-RC-Tool
- ✅ 不改 live repo
- ✅ 不做 destructive git 操作
- ✅ 不新增依赖
- ✅ 不做 method dispatch
- ✅ 不做 type inference
- ✅ 不做 overload resolution
- ✅ 不做 macro expansion
- ✅ 不开启新 slice

---

## Exit Criteria

- ✅ `cargo fmt --check` pass
- ✅ `cargo check --features tree-sitter-cangjie` clean (zero new warnings)
- ✅ `cargo test --features tree-sitter-cangjie` pass (0 fail)
- ✅ alias_reference tests: 9/9 pass (2 previously `#[ignore]` enabled)
- ✅ import_resolution tests: 10/10 pass
- ✅ cross_file_reference tests: 3/3 pass
- ✅ reference_extraction tests: 7/7 pass
- ✅ endpoint_integrity tests: 12/12 pass (0 dangling, 0 duplicate)
- ✅ Production smoke: 4/4 targets pass
- ✅ Confidence 策略已文档化
- ✅ Disambiguation 策略已文档化
- ✅ Dead code 已清理 (~100 lines removed)
- ✅ Warnings 已清理 (zero new Rust warnings)
- ✅ Closure review 完成
- ⏳ Commit + push（进行中）

**Hardening 状态：** ✅ 完成
