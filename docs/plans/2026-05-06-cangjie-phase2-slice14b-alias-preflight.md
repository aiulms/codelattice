# Cangjie Phase 2 Slice 14b — Alias Resolution Preflight

**Date:** 2026-05-06
**Type:** preflight（docs-only）
**Status:** 📝 Ready for execution
**Author:** aiulms
**Related Slices:** Slice 14a (completed: wildcard import expansion)

## 1. Phase 0 审计发现

### 1.1 当前已完成的 slices

| Slice | 描述 | 完成时间 | 关键技术 |
|-------|--------|---------|-----------|
| Slice 14a | Wildcard import expansion | 2026-05-06 | public modifier detection + wildcard expansion |

### 1.2 Alias import 当前状态

**✅ 已支持：**
- Simple alias import: `import pkg.func as f` → local name "f", exported name "func"
- Package alias: `import pkg as p` → package level alias
- AST detection: `PackageAlias` node in import AST walk
- `has_alias()` helper for simple alias detection
- `strip_alias()` helper for alias removal
- Import candidates parsing: 支持 alias in local_name field

**❌ 当前问题：**
1. **Grouped import alias 不支持**
   - `import pkg.{a, b as c}` → `parse_grouped_import()` 返回 `["a", "b as c"]`
   - `split_top_level_comma()` 处理含空格的符号：`"b as c"` 被完整作为一个 token
   - `is_valid_identifier("b as c")` 返回 false（含空格），所以被丢弃
   - **影响：** 无法处理 `import pkg.{a, b as c, d}` 形式的 alias

2. **Package alias 使用不完整**
   - AST 检测到 `PackageAlias` 节点，但后续 reference resolution 没有使用
   - `import pkg as p` → local name "p"，但 reference lookup 时仍然使用完整的 package name

### 1.3 可复用的现有基础设施

| 组件 | 文件 | 用途 |
|-------|------|------|
| Alias detection helpers | `imports.rs` | `has_alias()` / `strip_alias()` |
| Import candidates parser | `imports.rs` | `parse_named_import_candidates()` 支持 local_name |
| Package alias AST | `imports.rs` | `PackageAlias` 结构 + detection |
| Import binding table | `references.rs` | `ImportBindingTable` 支持 wildcard binding |
| Reference resolution | `references.rs` | `push_reference()` 优先使用 same-file → cross-file |

### 1.4 Root cause 分析

**Grouped import alias 失败的根本原因：**
1. **Tokenizer 层面**：`split_top_level_comma()` 按逗号分割，但 `b as c` 含空格，被当作一个 identifier
2. **Validation 层面**：`is_valid_identifier()` 对标识符定义过于严格，不允许 " as c" 格式（含空格）
3. **解析逻辑层面**：`parse_grouped_import()` 依赖 tokenizer，tokenizer 的问题传播到解析结果

**Package alias 失败的根本原因：**
1. **缺少 reference resolution 集成**：`PackageAlias` 节点检测后，`ImportBindingTable` 没有使用 package alias 信息
2. **binding lookup 逻辑**：`ImportBindingTable::resolve()` 只按 local_name 查找，不考虑 package prefix

## 2. MVP Scope

### 2.1 核心目标

修复 grouped import alias 支持，实现完整的 package alias 使用链路。

### 2.2 具体修复目标

**目标 1：修复 grouped import alias**
- 修复 `split_top_level_comma()` 支持 `"b as c"` 格式（含空格）
- 更新 `is_valid_identifier()` 允许 " as c" 格式的标识符
- 确保 grouped import 内的 alias 能正确解析

**目标 2：实现 package alias reference resolution**
- 扩展 `ImportBinding` 支持前缀映射：`package_prefix: Option<String>`
- 扩展 `ImportBindingTable::resolve()` 支持带前缀的 lookup
- 在 reference resolution 时使用前缀查找 bindings

### 2.3 实现边界

**✅ 在范围内：**
- Grouped import alias: `import pkg.{a, b as c}` → resolves to "a" and "c"
- Package alias: `import pkg as p` → can reference via "p.Func"
- Simple alias: 继续正常工作

**❌ 超出范围（stop-lines）：**
- Full alias chaining（嵌套 alias）：不支持（过于复杂）
- Multi-level package alias: 不支持（如 `import pkg as p` → `p as q`）
- Macro alias expansion: 不支持（宏展开超出范围）

### 2.4 Write Set

| 文件 | 变更类型 | 预估行数 |
|------|---------|----------|
| `crates/cangjie/src/extractors/imports.rs` | 修改 | ~80 行 |
| `crates/cangjie/src/extractors/references.rs` | 修改 | ~50 行 |
| `crates/cangjie/src/extractors/graph.rs` | 修改 | ~10 行 |
| `crates/cangjie/tests/alias_reference.rs` | 新增 | ~150 行 |
| `docs/plans/2026-05-06-cangjie-phase2-slice14b-alias-preflight.md` | 新增 | 本文档 |

### 2.5 Forbidden Write Set

- GitNexus-RC runtime/schema/package/web — ❌ 不修改
- GitNexus-RC-Tool — ❌ 不修改
- Live repos — ❌ 不修改
- LSP — ❌ 不实现
- MCP/HTTP/UI — ❌ 不实现
- Type inference/trait solving — ❌ 不实现
- Macro expansion — ❌ 不实现

## 3. 技术方案

### 3.1 修复 grouped import alias

**方案 A：修复 tokenizer，支持含空格的 alias**
```rust
// 在 split_top_level_comma() 中，更严格的分词
fn split_top_level_comma(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut in_alias = false;
    
    for (i, ch) in s.char_indices() {
        if ch == ',' {
            in_alias = false;
            continue;
        }
        
        if ch == ' ' ' {
            continue;
        }
        
        tokens.push(ch.to_string());
        
        if in_alias {
            if ch == 'a' && i + 1 < s.len() && s.chars().nth(i + 1) == 's' && i + 2 < s.len() && s.chars().nth(i + 2) == ' ' {
                // "a as x" 格式，允许 " as c" 后有空格
                tokens.push(ch.to_string());
            }
        }
    }
    
    tokens
}
```

**方案 B：更新 identifier validation**
```rust
fn is_valid_identifier(id: &str) -> bool {
    // 更宽松的规则，允许 " as c" 格式（含空格）
    let parts: Vec<&str> = id.split_whitespace().collect();
    
    // 简单检查：非空、不包含特殊字符、不以数字开头
    if parts.is_empty() {
        return false;
    }
    
    let first = parts[0];
    
    // 检查是否是保留字（作为最后一个 token）
    if matches!(first, "as|import|from|export") {
        return parts.len() > 1; // 允许 "a as b", "import pkg"
    }
    
    true
}
```

### 3.2 实现 package alias reference resolution

**方案 A：扩展 ImportBinding 结构**
```rust
pub struct ImportBinding {
    pub target_file: String,
    pub target_name: String,
    pub is_wildcard: bool,
    pub package_prefix: Option<String>, // 新增：package alias 前缀
}
```

**方案 B：扩展 ImportBindingTable resolve**
```rust
pub fn resolve(&self, source_file: &str, name: &str) -> Option<&ImportBinding> {
    let candidates = self.bindings.get(&(source_file.to_string(), name.to_string()))?;
    
    // 先尝试精确匹配（without prefix）
    if candidates.len() == 1 {
        return Some(&candidates[0]);
    }
    
    // 尝试带前缀的匹配（package alias case）
    for candidate in candidates {
        if let Some(prefix) = &candidate.package_prefix {
            if name.starts_with(prefix) {
                let local_name = &name[prefix.len()..];
                if local_name == "" {
                    return Some(candidate); // "import pkg as p", local name "" → matches
                }
            }
        }
    }
    
    None
}
```

**方案 C：修改 reference resolution 优先级**
```rust
// 在 push_reference() 中调整优先级
fn push_reference() {
    // 优先级 1: same-file (SameFileIndex)
    // 优先级 2: explicit import (ImportBindingTable, exact match)
    // 优先级 3: package alias (ImportBindingTable, with prefix)
}
```

## 4. Acceptance Criteria

| AC | 验证方式 | 预期结果 |
|----|---------|---------|
| AC1: `import pkg.{a, b as c}` → resolves to "a" and "c" | 集成测试 | ✅ |
| AC2: `import pkg as p` → can reference "p.Func" | 集成测试 | ✅ |
| AC3: Simple alias continues to work | 集成测试 | ✅ |
| AC4: No regressions in existing functionality | 集成测试 | ✅ |
| AC5: All tests pass (unit + integration) | 运行测试 | ✅ |
| AC6: cargo fmt --check pass | 格式检查 | ✅ |
| AC7: No new dependencies | 依赖检查 | ✅ |
| AC8: No forbidden writes | 代码审计 | ✅ |

## 5. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| Backward compatibility | MEDIUM | 保持 simple alias 和 package alias 的向后兼容 |
| Tokenizer complexity | LOW | 简化分词逻辑，增加单元测试覆盖 |
| Binding lookup ambiguity | LOW | 严格匹配规则， ambiguous → no edge |
| Performance impact | LOW | 额外的 map 查找，但 O(1) 复杂度可接受 |
| Test coverage gap | MEDIUM | 需要覆盖 grouped import + package alias 组合场景 |

## 6. 依赖关系

**新依赖：** ❌ 无（仅现有基础设施）

**依赖变更：**
- `ImportBinding` 结构：新增 `package_prefix` 字段
- `ImportBindingTable::resolve()`: 新增前缀匹配逻辑
- Tokenizer/Validator: 更新 `split_top_level_comma()` 和 `is_valid_identifier()`
- Reference resolution: 调整 `push_reference()` 优先级

## 7. 执行策略

**Phase 1: 修复 tokenizer** (15 min)
1. 更新 `split_top_level_comma()` 支持 " as c" 格式
2. 添加单元测试验证分词逻辑

**Phase 2: 更新 validation** (10 min)
1. 更新 `is_valid_identifier()` 允许含空格的 alias

**Phase 3: 扩展 binding 结构** (5 min)
1. 添加 `package_prefix` 字段到 `ImportBinding`

**Phase 4: 实现 binding lookup** (20 min)
1. 修改 `ImportBindingTable::resolve()` 支持前缀匹配

**Phase 5: 创建 fixture** (10 min)
1. 新增 `fixtures/cangjie/alias-basic/` fixture
2. 包含 grouped alias, package alias, simple alias

**Phase 6: 编写测试** (30 min)
1. 新增 `crates/cangjie/tests/alias_reference.rs`
2. 覆盖所有 alias 场景

**Phase 7: 集成测试** (10 min)
1. 运行完整测试套件，验证无回归

**Phase 8: 文档** (5 min)
1. 写 execution card
2. 更新 closure review

**总估计时间：** ~95 min

## 8. 下一步推荐

**推荐：** ✅ **进入 execution card**

Slice 14b 的技术路径清晰，风险评估充分，所有依赖都是现有基础设施。建议继续执行，在 bounded slice 内完成 alias resolution 功能。

## 9. Stop-lines 重申

以下内容是 Rust-core MVP 的明确 stop-line：

- **No production replacement** — Rust-core 不是 GitNexus-RC TypeScript adapter 的替代
- **No LSP client** — 仅实现 basic reference resolution
- **No MCP/HTTP/UI** — 不实现服务层
- **No type inference / trait solving** — 不推断类型
- **No macro expansion** — 不展开宏
- **No full package alias** — 仅支持单级 prefix，不支持嵌套
- **No full alias chaining** — 不支持 `import pkg as p as q` 嵌套

**新增：** alias resolution 中不支持的复杂场景
- Nested package alias: `import pkg as p as q`
- Multi-level alias: 超出 MVP 范围
- Macro alias: 超出 static analysis 范围
