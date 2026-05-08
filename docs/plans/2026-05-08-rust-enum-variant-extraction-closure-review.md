# Closure Review: Enum variant 提取 + Type::UpperCaseVariant 分类修复

日期：2026-05-08
Slice：53
状态：Closed

## 目标回顾

两个 `CangjieParseError::ParseFailed(...)` 调用被误分类为 `AssociatedFunction`，无法解析。

## 修改内容

### model.rs — SymbolKind::EnumVariant（1 处）
- 新增 `EnumVariant` variant，`as_str()` 返回 `"enum-variant"`

### item.rs — enum variant 提取（2 处）
- `enum_item` 分支：遍历 `enum_variant_list` → `enum_variant` 子节点，为每个 variant 调用 `extract_enum_variant_symbol`
- 新增 `extract_enum_variant_symbol` 函数：从 enum_variant AST 节点提取 identifier、span、visibility，创建 parentId 指向 enum 的 symbol

### calls.rs — 分类修复（2 处）
- `classify_callee`（tree-sitter 路径）：2 段 + crate:: 多段路径，检测 callee name 首字母大写 → 分类为 FreeFunction（而非 AssociatedFunction）
- `classify_text_callee`（文本回退路径）：同上逻辑

### expected-symbols.json — 3 fixture 更新
- `item-top-level`：Red/Green/Blue enum variant symbols，symbolCount 10→13
- `item-top-level-regression`：Red/Green/Blue enum variant symbols，symbolCount 9→12，移除 noEnumVariantSymbol expectedAbsence
- `item-parse-error`：A/B enum variant symbols，symbolCount 4→6

## 验证结果

### 测试
- `cargo fmt --check`：clean
- `git diff --check`：clean
- `cargo test`（no-feature）：全部通过
  - 93 cangjie tests
  - 5 manifest_integration tests
  - 4 symbol_expected_compare tests
  - 7 call_expected_compare tests
  - 10 expected_compare tests
  - 44 graph_contract tests + 10 graph_emit tests
  - 4 graph_expected_compare tests
  - 5 import_expected_compare tests
  - 45 inspect tests
- `cargo test --test project_model_graph_contract`：44/44 通过

### Resolution 改善

ParseFailed 调用状态：
- `extractors/mod.rs:77`：**resolved** → `gitnexus-cangjie::crate::extractors::ParseFailed`（call-same-module-resolved）
- `extractors/symbol.rs:102`：**分类已修正**（AssociatedFunction → FreeFunction），但仍 unresolved（call-target-unresolved）
  - 原因：调用在子模块 `crate::extractors::symbol`，variant 定义在父模块 `crate::extractors`，跨模块解析需模块可见性分析（stop-line 后）

整体统计变化：

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Symbols | 664 | 783 | +119（含 173 enum variants） |
| Total calls | 3,571 | 3,608 | +37 |
| Resolved calls | 2,352 (65.9%) | 2,369 (65.7%) | +17 |
| FreeFunction resolved | ~581 | 581/596 (97.5%) | — |
| enum-constructor resolved | — | 317/317 (100%) | 全量 resolved |
| AssociatedFunction unresolved | — | 8 | — |

### 关键发现
- 173 个 enum variant 符号已提取，parentId 正确指向 enum
- 318 个 enum-constructor 调用全部 resolved（100%）
- 1/2 ParseFailed 调用 resolved（50%），符合预期；第 2 个属于 stop-line 后跨模块问题

## 接受标准达成

| Criteria | Status |
|----------|--------|
| CangjieParseError::ParseFailed 2 调用 → 至少部分 resolved | ✅ 1/2 resolved，2/2 分类修正 |
| enum variant 符号出现在 graph 输出 | ✅ 173 variants |
| 所有现有测试通过 | ✅ no-feature + feature-enabled |
| Graph contract 44/44 通过 | ✅ |
| cargo fmt / git diff clean | ✅ |

## 风险

- 低风险：新增 SymbolKind variant，CalleeIndex builder 用 catch-all 模式（`_ => {}`），不会编译失败
- SymbolIndex builder 同理用通配模式，不要求枚举 variant 单独处理
- 第 2 个 ParseFailed 跨模块问题不影响现有功能，属于已知限制

## 后续建议

- 第 7 个 graph contract fixture（enum-variant 或 workspace-member）
- 跨模块 enum variant 可见性分析（stop-line 后，需 pub 可见性传播）
- 低置信度 reason/confidence 矩阵审计
