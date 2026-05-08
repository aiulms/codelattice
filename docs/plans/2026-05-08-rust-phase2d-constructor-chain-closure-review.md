# Rust Phase 2d — let-binding 构造函数链 receiver type 推断 closure review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 2 — Rust CALLS resolution quality
**Commit：** 待提交

---

## 总结

本轮实现 Phase 2d：扩展 `scan_variable_type_annotation` 以支持 let-binding 构造函数链 receiver type 推断。当变量无显式类型注解但 RHS 为已知 stdlib 构造函数时（例如 `let v = Vec::new(); v.push(1)`），通过构造函数推断变量类型并解析后续方法调用。

## 变更内容

### 1. KNOWN_CONSTRUCTORS 表 + lookup_constructor_type 函数

`stdlib_tables.rs` 新增：

```rust
const KNOWN_CONSTRUCTORS: &[(&str, &str)] = &[
    ("Vec::new", "Vec"),
    ("Vec::with_capacity", "Vec"),
    ("String::new", "String"),
    ("String::from", "String"),
    ("HashMap::new", "HashMap"),
    ("HashMap::with_capacity", "HashMap"),
    ("HashSet::new", "HashSet"),
    ("PathBuf::new", "PathBuf"),
    ("PathBuf::from", "PathBuf"),
    ("BTreeMap::new", "BTreeMap"),
];

pub(crate) fn lookup_constructor_type(constructor_path: &str) -> Option<&'static str>;
```

覆盖 10 个最常见 stdlib 构造函数 → 6 种基础类型。

### 2. Phase 2d 逻辑 — scan_variable_type_annotation 扩展

在 `scan_variable_type_annotation` 中新增第三阶段扫描（2d）：

1. **Phase 2a（已有）：** 扫描 `let var: Type = ...` 显式类型注解
2. **Phase 2b（已有）：** 扫描函数参数类型注解
3. **Phase 2d（新增）：** 扫描 `let var = Constructor(...)` 无类型注解但 RHS 为已知构造函数的声明，通过 `lookup_constructor_type` 推断变量类型

关键实现：
- 只在 Phase 2a 和 2b 均未匹配时才触发 2d
- 仅匹配 `let var = <path>(...)` 或 `let mut var = <path>(...)` 模式
- 构造函数路径通过 `lookup_constructor_type` 映射到基础类型
- 基础类型再通过 `lookup_receiver_type_method` 查找 method 路径
- confidence 保持 0.65（与显式类型注解 method 相同）
- 不涉及类型推断 — 仅利用已知构造函数→基础类型的静态映射

### 3. 测试夹具 c12-let-constructor-method

新增 `fixtures/call-resolution/c12-let-constructor-method/`：
- 5 个函数覆盖 Vec::new/with_capacity, String::new/from, HashMap::new
- 14 calls：5 构造函数 + 9 方法调用
- 全部 9 个方法调用通过 Phase 2d 解析
- compile-valid fixture

### 4. strip_generics 恢复

修复了一个回归：`strip_generics` 在之前 KNOWN_CONSTRUCTORS 表添加时被误删除（被替换），现已恢复。该函数仍在 `calls.rs` 中被 4 处调用。

## 效果测量

在 gitnexus-rust-core 自身（~3500 calls）上测量：

| 指标 | Before | After | 变化 |
|------|--------|-------|-------|
| Total calls | 3500 | 3514 | +14 (含 c12 fixture) |
| Resolved | 2178 (62.2%) | 2252 (64.1%) | +74 (+1.9pp) |
| Unresolved | 1322 (37.8%) | 1262 (35.9%) | -60 |
| receiver-type-method-resolved | 164 | 235 | +71 |

+71 个 receiver-type-method-resolved 全部来自构造函数链推断：`let v = Vec::new(); v.push(1)` 等模式。

## Stop-lines 合规

- ✅ 未做 type inference（仅静态构造函数→基础类型映射）
- ✅ 未做 trait solving
- ✅ 未做 macro expansion
- ✅ 未做 full cfg evaluator
- ✅ 未新增依赖
- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ confidence 保持 0.65（不提高，因为构造函数链推断不如显式类型注解可靠）

## 验证结果

- `cargo fmt --check` ✅ clean
- `git diff --check` ✅ clean
- `cargo test` ✅ 全部通过
- `cargo test --features tree-sitter-cangjie` ✅ 全部通过
- `project_model_call_expected_compare` ✅ 20/20 fixtures pass（含新 c12）
- `graph_contract` ✅ 24/24 pass
- `multi_project_smoke` ✅ 4/4 fixture pass
- `cangjie_inspect` ✅ 18/18 pass

## 已知限制

1. **仅 stdlib 构造函数** — KNOWN_CONSTRUCTORS 表仅含 10 个 stdlib 构造函数，扩展需要手动添加
2. **单层链** — 仅推断 `let v = Constructor(); v.method()`，不支持 `let v = some_fn(); v.method()` 或 `let v = other_var; v.method()`
3. **不支持泛型实例化** — `let v: Vec<String> = Vec::new();` 走 Phase 2a（有类型注解），`let v = Vec::<String>::new(); v.push("x")` 可能不匹配 constructor_path 验证
4. **HashSet/PathBuf/BTreeMap 无 STDLIB_TYPE_METHODS 条目** — 即使推断出类型也无方法解析；需要时可补 entry

## 下一轮 Opening

继续 Priority 2 — Rust CALLS resolution quality：
- crate::/self::/super:: path resolution edge case 修复
- low-confidence reason/confidence 矩阵审计
- 或 Priority 1 — Rust production readiness smoke（对更多真实 Rust 项目 smoke）
