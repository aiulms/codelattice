# Closure Review: crate:: 多段路径 AssociatedFunction 误分类修复

日期：2026-05-08
状态：完成
关联 Preflight：`docs/plans/2026-05-08-rust-crate-path-associated-fn-misclassification-preflight.md`

## Landed Reality

### 修改内容

在 `crates/project-model/src/calls.rs` 中修改了两处 `crate::` 路径分类逻辑：

1. **`classify_callee`（tree-sitter 版本）** 第 882-883 行：
   当 `first == "crate"` 时，新增检查：如果 `segments.len() >= 4` 且倒数第二段首字母大写，
   则分类为 `AssociatedFunction`，否则保持 `QualifiedPath`。

2. **`classify_text_callee`（文本 fallback 版本）** 第 1905-1906 行：
   同上逻辑。

### 新增测试

- 新增 fixture `c16-crate-associated-fn`（compile-valid，2 source files，3 calls）
  - `crate::inner::MyType::build("test")` → 正确分类为 `associated-function`，callKind 验证
  - `crate::inner::helper(10)` → 正确分类为 `qualified-path`（不退化）
  - `name.to_string()` → stdlib trait method（不受影响）

### 实际效果

- `CrossFileSymbolIndex::build()` 的 `crate::` 路径调用：从 incorrectly classified `QualifiedPath` → 正确分类 `AssociatedFunction` 并成功解析
- `associated-function resolved`: 1 → 2 (+1)
- `unresolved associated-function`: 16 → 15 (-1)
- `unresolved qualified-path`: 8 → 7 (-1)
- Resolution rate: 65.8%（变化太小不显示，+1 resolved among ~3563 total）

### 验证结果

- `cargo fmt --check`: ✅ clean
- `git diff --check`: ✅ clean
- `cargo test`（no-feature）: ✅ 全部通过，0 fail
- `cargo test --features tree-sitter-cangjie`: ✅ 全部通过，0 fail
- `project_model_graph_contract`: ✅ 30/30 pass
- `project_model_call_expected_compare`: ✅ 7/7 pass（含新 c16 fixture）
- Cangjie production gate: 未被触碰

### Stop-line 合规

- No type inference / trait solving ✅
- No macro expansion ✅
- No external crate resolution ✅
- No destructive git ✅
- No new dependencies ✅
- No GitNexus-RC / Tool / live repo modification ✅

### 残留风险

- **LOW**：修改仅影响分类，不改变解析逻辑。`resolve_associated_function` 已有处理 `crate::` 前缀路径的能力（分支 B）。
- 对于没有大写倒数第二段的 `crate::` 路径（如 `crate::module::sub::function()`），行为不变。

### 下一步

来自 docs/plans/README.md 当前未完成 openings：
1. 关联函数 resolution：还有 15 unresolved（含 derive-generated 方法、外部 crate type 方法、re-export 路径等）。部分超出 stop-line，需逐个分析。
2. 低置信度 reason/confidence 矩阵审计
3. Rust graph contract 第 5 个 fixture
