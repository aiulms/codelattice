# Cangjie Phase 2 Slice 15 — Wildcard Import Edge Quality

**Date:** 2026-05-06
**Type:** preflight（docs-only）
**Status:** 📝 Ready for execution
**Author:** aiulms
**Related Slices:** Slice 14a (completed: wildcard import expansion), Slice 14b (completed: alias resolution)

## 1. Phase 0 审计发现

### 1.1 当前已完成的 slices

| Slice | 描述 | 完成时间 | 关键技术 |
|-------|--------|---------|-----------|
| Slice 14a | Wildcard import expansion | 2026-05-06 | public modifier detection + wildcard expansion |
| Slice 14b | Alias resolution | 2026-05-06 | grouped alias + package alias support |

### 1.2 Wildcard import 当前状态

**✅ 已支持：**
- Wildcard import detection: `import pkg.*`
- Public symbol extraction: `is_public: bool` in `CangjieSymbol`
- Wildcard import expansion: `public_symbols_in_dir()` method
- Import binding creation: `ImportBinding` with `is_wildcard: bool`
- Confidence distinction: wildcard (0.75) vs explicit (0.85)

**❌ 当前问题：**
1. **Wildcard edge quality 不够精确**
   - 当前所有 wildcard 导入的符号都产生相同 confidence (0.75)
   - 没有考虑符号命名冲突的可能性
   - 没有区分常见符号 vs 稀有符号

2. **Ambiguity 缺少 guard**
   - 当多个包导出同名符号时，wildcard import 容易产生歧义
   - 当前实现没有检测和处理这种冲突
   - 可能产生低质量或错误的 reference edges

3. **No cross-package conflict detection**
   - `import pkg1.*` 和 `import pkg2.*` 如果都导出 `Func`，无法区分
   - 需要更智能的符号选择策略

### 1.3 可复用的现有基础设施

| 组件 | 文件 | 用途 |
|-------|------|------|
| Wildcard detection | `imports.rs` | `is_wildcard` field detection |
| Public symbol extraction | `symbol.rs` | `is_public` field + detection logic |
| Wildcard expansion | `references.rs` | `public_symbols_in_dir()` method |
| Confidence scoring | `references.rs` | Existing confidence framework |
| Import binding | `references.rs` | `ImportBinding` structure + table |

### 1.4 Root cause 分析

**Wildcard edge quality 不够的根本原因：**
1. **平权策略**：所有 wildcard 导入符号使用相同 confidence，没有区分质量
2. **缺少冲突检测**：没有检查同名符号来自不同包的情况
3. **符号选择简单**：仅依赖 `first()` 或简单遍历，没有优先级策略

**Ambiguity 缺少 guard 的根本原因：**
1. **静态分析局限**：无法知道实际运行时使用哪个符号
2. **缺少启发式规则**：没有使用命名约定、导入顺序等线索
3. **置信度单调**：wildcard confidence (0.75) 固定，没有动态调整

## 2. MVP Scope

### 2.1 核心目标

提升 wildcard import reference edges 的质量和可靠性，添加冲突检测和歧义保护。

### 2.2 具体改进目标

**目标 1：实现符号冲突检测**
- 检测同名符号是否来自多个包
- 当检测到冲突时，降低 confidence 或产生 no-edge
- 添加冲突统计到 binding metadata

**目标 2：改进 wildcard confidence scoring**
- 基于符号稀有度调整 confidence
- 考虑符号命名模式（如 `Common*` vs `Specific*`）
- 区分显式导入 vs 隐式 wildcard 的优先级

**目标 3：添加歧义 guard**
- 当 reference 解析存在多个候选时，应用启发式规则
- 优先选择：显式导入 > 最近的 wildcard 导入 > 命名匹配度
- 无法解决歧义时，返回 None 而非猜测

### 2.3 实现边界

**✅ 在范围内：**
- Wildcard import 符号冲突检测
- 基于 rarity 和命名模式的 confidence 调整
- 基本启发式歧义解决规则
- Cross-package 同名符号检测
- Wildcard edge 质量统计和日志

**❌ 超出范围（stop-lines）：**
- Full type-based disambiguation（需要 type inference）
- Runtime usage analysis（超出 static analysis）
- Import statement 顺序分析（需完整解析）
- User preference / IDE integration（LSP scope）
- Complex semantic conflict resolution（需要完整编译信息）

### 2.4 Write Set

| 文件 | 变更类型 | 预估行数 |
|------|---------|----------|
| `crates/cangjie/src/extractors/references.rs` | 修改 | ~120 行 |
| `crates/cangjie/src/extractors/symbol.rs` | 修改 | ~20 行 |
| `crates/cangjie/tests/wildcard_edge_quality.rs` | 新增 | ~180 行 |
| `docs/plans/2026-05-06-cangjie-phase2-slice15-wildcard-edge-quality-preflight.md` | 新增 | 本文档 |

### 2.5 Forbidden Write Set

- GitNexus-RC runtime/schema/package/web — ❌ 不修改
- GitNexus-RC-Tool — ❌ 不修改
- Live repos — ❌ 不修改
- LSP — ❌ 不实现
- MCP/HTTP/UI — ❌ 不实现
- Type inference/trait solving — ❌ 不实现
- Macro expansion — ❌ 不实现

## 3. 技术方案

### 3.1 符号冲突检测

**方案 A：Cross-package 符号索引**
```rust
// 在 CrossFileSymbolIndex 中添加冲突检测
impl CrossFileSymbolIndex {
    /// Detect if a symbol name appears in multiple packages
    pub fn detect_symbol_conflicts(&self, name: &str) -> Vec<SymbolConflict> {
        let mut conflicts = Vec::new();
        for (file, symbols) in &self.symbols_by_file {
            for symbol in symbols {
                if symbol.name == name {
                    conflicts.push(SymbolConflict {
                        file: file.clone(),
                        package: extract_package_name(file),
                    });
                }
            }
        }
        conflicts
    }
}
```

**方案 B：Binding table 冲突检测**
```rust
// 在 ImportBindingTable::build() 中添加冲突检测
pub fn build(...) -> Self {
    // ... existing code ...

    // Detect conflicts for wildcard bindings
    for ((source_file, _), bindings) in &bindings {
        let conflicts = detect_wildcard_conflicts(bindings);
        if !conflicts.is_empty() {
            // Mark conflicting bindings with lower confidence
            for binding in bindings {
                if binding.is_wildcard {
                    binding.confidence *= 0.8; // Penalty for conflict
                }
            }
        }
    }
}
```

### 3.2 改进 wildcard confidence scoring

**方案 A：基于符号稀有度的动态 confidence**
```rust
pub fn calculate_wildcard_confidence(
    symbol: &CangjieSymbol,
    total_symbols: usize,
    name_frequency: &HashMap<String, usize>,
) -> f64 {
    let base_confidence = 0.75;

    // Rare symbols get higher confidence
    let frequency = name_frequency.get(&symbol.name).unwrap_or(&1);
    let rarity_bonus = if *frequency == 1 {
        0.10 // Bonus for unique symbols
    } else if *frequency <= 3 {
        0.05 // Small bonus for rare symbols
    } else {
        -0.05 // Penalty for common symbols
    };

    (base_confidence + rarity_bonus).min(0.85).max(0.60)
}
```

**方案 B：基于命名模式的启发式规则**
```rust
pub fn adjust_confidence_by_naming(
    symbol_name: &str,
    is_wildcard: bool,
) -> f64 {
    let base_confidence = if is_wildcard { 0.75 } else { 0.85 };

    // Specific naming patterns suggest better quality
    let pattern_bonus = if symbol_name.contains("Specific") ||
                        symbol_name.contains("Custom") ||
                        symbol_name.len() > 8 {
        0.05 // Bonus for specific names
    } else if symbol_name.starts_with("Common") ||
                symbol_name.starts_with("Generic") {
        -0.05 // Penalty for generic names
    } else {
        0.0
    };

    (base_confidence + pattern_bonus).min(0.85).max(0.60)
}
```

### 3.3 添加歧义 guard

**方案 A：显式导入优先策略**
```rust
pub fn resolve_with_disambiguation(
    &self,
    source_file: &str,
    name: &str,
    explicit_imports: &[String],
) -> Option<&ImportBinding> {
    // Priority 1: Exact match with explicit import
    if let Some(binding) = self.find_explicit_match(source_file, name) {
        return Some(binding);
    }

    // Priority 2: Unique wildcard match
    if let Some(binding) = self.find_unique_wildcard_match(source_file, name) {
        return Some(binding);
    }

    // Priority 3: Ambiguous wildcard → apply heuristics or return None
    let wildcard_matches = self.find_all_wildcard_matches(source_file, name);
    if wildcard_matches.len() > 1 {
        return self.apply_heuristics(wildcard_matches, name);
    }

    None
}
```

**方案 B：启发式歧义解决**
```rust
pub fn apply_heuristics(
    &self,
    candidates: Vec<&ImportBinding>,
    name: &str,
) -> Option<&ImportBinding> {
    // Heuristic 1: Prefer symbols with matching prefixes
    let prefix_matches: Vec<_> = candidates.iter()
        .filter(|b| b.target_name.starts_with(name))
        .collect();

    if prefix_matches.len() == 1 {
        return Some(prefix_matches[0]);
    }

    // Heuristic 2: Prefer symbols from more specific packages
    let specificity_scores: Vec<_> = candidates.iter()
        .map(|b| (b, calculate_specificity_score(b)))
        .collect();

    let best = specificity_scores.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    best.map(|(binding, _)| *binding)
}
```

## 4. Acceptance Criteria

| AC | 验证方式 | 预期结果 |
|----|---------|---------|
| AC1: Detect symbol conflicts across packages | 集成测试 | ✅ |
| AC2: Reduce confidence for conflicting wildcard symbols | 集成测试 | ✅ |
| AC3: Apply rarity-based confidence adjustment | 集成测试 | ✅ |
| AC4: Implement disambiguation with explicit import priority | 集成测试 | ✅ |
| AC5: Return None for unresolvable ambiguity | 集成测试 | ✅ |
| AC6: No regressions in existing wildcard functionality | 集成测试 | ✅ |
| AC7: All tests pass (unit + integration) | 运行测试 | ✅ |
| AC8: cargo fmt --check pass | 格式检查 | ✅ |
| AC9: No new dependencies | 依赖检查 | ✅ |
| AC10: No forbidden writes | 代码审计 | ✅ |

## 5. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| Over-engineering disambiguation | MEDIUM | Keep heuristics simple, document limitations |
| Performance impact from conflict detection | LOW | Cache conflict results, lazy evaluation |
| False positive conflict detection | LOW | Use conservative thresholds, add tests |
| Backward compatibility | LOW | Maintain existing API, additive changes only |
| Test coverage complexity | MEDIUM | Focus on edge cases, use parametrized tests |

## 6. 依赖关系

**新依赖：** ❌ 无（仅现有基础设施）

**依赖变更：**
- `CrossFileSymbolIndex`：添加冲突检测方法
- `ImportBinding`：可能添加 `conflict_count: usize` 字段
- `ImportBindingTable::resolve()`：添加歧义解决逻辑
- Confidence 计算：添加动态调整函数
- Wildcard expansion：集成冲突检测

## 7. 执行策略

**Phase 1: 实现符号冲突检测** (20 min)
1. 添加 `detect_symbol_conflicts()` 到 `CrossFileSymbolIndex`
2. 添加 `SymbolConflict` 结构定义
3. 编写冲突检测单元测试

**Phase 2: 改进 confidence scoring** (15 min)
1. 实现 `calculate_wildcard_confidence()` 函数
2. 添加基于命名模式的调整逻辑
3. 集成到 wildcard import expansion 流程

**Phase 3: 实现歧义 guard** (25 min)
1. 修改 `ImportBindingTable::resolve()` 支持优先级
2. 实现显式导入优先策略
3. 添加启发式歧义解决规则

**Phase 4: 创建 fixture** (10 min)
1. 新增 `fixtures/cangjie/wildcard-conflicts/` fixture
2. 包含多包同名符号、显式 vs wildcard 混合场景

**Phase 5: 编写测试** (40 min)
1. 新增 `crates/cangjie/tests/wildcard_edge_quality.rs`
2. 覆盖冲突检测、confidence 调整、歧义解决
3. 测试边界情况

**Phase 6: 集成测试** (10 min)
1. 运行完整测试套件，验证无回归
2. 确认现有 wildcard 功能不受影响

**Phase 7: 文档** (5 min)
1. 写 execution card
2. 写 closure review

**总估计时间：** ~125 min

## 8. 下一步推荐

**推荐：** ✅ **进入 execution card**

Slice 15 的技术路径清晰，风险评估充分，所有依赖都是现有基础设施。建议继续执行，在 bounded slice 内完成 wildcard import edge quality 改进。

## 9. Stop-lines 重申

以下内容是 Rust-core MVP 的明确 stop-line：

- **No production replacement** — Rust-core 不是 GitNexus-RC TypeScript adapter 的替代
- **No LSP client** — 仅实现 basic reference resolution
- **No MCP/HTTP/UI** — 不实现服务层
- **No type inference / trait solving** — 不推断类型
- **No macro expansion** — 不展开宏
- **No full semantic disambiguation** — 仅使用启发式规则，不依赖完整编译信息
- **No runtime analysis** — 仅 static analysis，不分析运行时行为

**新增：** wildcard edge quality 中不支持的复杂场景
- Type-based disambiguation: 超出 static analysis 范围
- Runtime usage statistics: 需要运行时数据收集
- User preference learning: 超出 MVP 范围
- Complex semantic analysis: 需要完整编译器前端
