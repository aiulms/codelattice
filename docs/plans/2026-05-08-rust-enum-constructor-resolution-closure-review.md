# Rust Enum Constructor Resolution — Closure Review

**日期：** 2026-05-08
**状态：** 完成
**类型：** Priority 2 — Rust CALLS Resolution Quality
**Commit：** `d9f5997`

---

## 总结

将 Rust stdlib enum variant constructor（Some/Ok/Err/None）从"过滤为不可解析"改为"解析到已知 enum variant symbol ID"。305 个枚举构造函数调用从 unresolved 变为 resolved，resolution rate 从 53.7% 提升到 62.4%（+8.7 个百分点）。

## 变更内容

### model.rs — 新增 `CallKnownEnumConstructor` 枚举变体

- 新增 `CallResolutionReason::CallKnownEnumConstructor` 变体
- as_str() 映射为 `"call-known-enum-constructor"`
- confidence 0.80：名称已知但未做 receiver type 验证，低于 same-module(0.90)

### calls.rs — 枚举构造函数解析替换过滤逻辑

- 新增 `KnownEnumVariant` struct 和 `resolve_known_enum_constructor()` 函数
- 支持的映射：
  - `Some` → `std::option::Option::Some`
  - `None` → `std::option::Option::None`
  - `Ok` → `std::result::Result::Ok`
  - `Err` → `std::result::Result::Err`
- 替换了原来的硬编码过滤块（`RUST_ENUM_CONSTRUCTORS`）
- callKind 改为 `"enum-constructor"`，knownCrate 设为 `"std"`

### Golden fixture 更新

- `call-enum-filter/expected-calls.json`：4 个条目更新（Some/Ok/Err × 2 Ok）
- `c11-receiver-type/expected-calls.json`：2 个条目更新（Some/Ok）

## gitnexus-rust-core 自身测量

| 指标 | 修改前 | 修改后 | 变化 |
|------|--------|--------|------|
| Total calls | 3500 | 3500 | 不变 |
| Resolved | ~1878 | 2183 | +305 |
| Unresolved | ~1622 | 1317 | -305 |
| Resolution rate | 53.7% | 62.4% | +8.7pp |
| Enum constructors resolved | 0 | 305 | +305 |

### Reason 分布（修改后）

| Reason | Count |
|--------|-------|
| call-target-unresolved | 1259 |
| call-stdlib-trait-method-resolved | 896 |
| call-same-module-resolved | 472 |
| call-known-enum-constructor | 305 |
| call-external-crate-path-resolved | 205 |
| call-receiver-type-method-resolved | 164 |
| Others | 199 |

## 设计决策

- **不扩到自定义 enum**：只解析已知 stdlib enum variant（名称在 Rust 生态中唯一且无歧义）
- **不验证 receiver type**：不检查 `Some(42)` 是否真的返回 `Option<i32>`（需要 type inference，stop-line）
- **confidence 0.80**：低于 same-module(0.90)，高于盲 method-name(0.65)，因为名称已知但未做类型验证
- **保留 CallEnumConstructor 变体**：原 `CallEnumConstructor` 变体保留在枚举中，用于未来可能的非 stdlib enum constructor 标记

## 验证结果

- `cargo fmt --check`: clean
- `git diff --check`: clean
- `cargo test` (no-feature): 全部通过（cangjie 93/93, project_model 4/4, cli tests 全部通过）
- `cargo test --features tree-sitter-cangjie`: 全部通过（cangjie 112/112, cangjie_inspect 18/18, graph_contract 24/24, multi_project_smoke 4/4, project_model call comparison 19/19 fixtures pass）
- Rust graph contract: 8/8 pass
- Rust graph emit: 10/10 pass

## Stop-lines 合规

- ✅ 未做 type inference / trait solving
- ✅ 未做 macro expansion
- ✅ 未做 full cfg evaluator
- ✅ 未新增依赖
- ✅ 未修改 GitNexus-RC / Tool / live repo
- ✅ 未做 destructive git 操作

## Residual Risks

- **已知限制**：自定义 enum variant constructor（如 `MyEnum::Variant(x)`）仍然不可解析，因为它们不在已知映射表中
- **Receiver type 未验证**：`Some(42)` 解析到 `std::option::Option::Some` 是正确的，但没有验证调用上下文中确实期望 Option 类型
- **None 未被实际触发**：在 gitnexus-rust-core 中没有 `None` 调用，但映射已在代码中支持

## 下一轮 Opening

Priority 2 后续方向（按价值排序）：

1. **crate::/self::/super:: path resolution 改善**：当前已有基本支持（confidence 0.80-0.90），可以检查是否还有遗漏的 edge case
2. **Same-file qualified call 修复**：`module::function()` 形式的调用解析，当前 confidence 0.85
3. **External crate classification 扩展**：识别更多 stdlib type path（当前已有 sysroot index ~90 条目）
4. **Low-confidence reason/confidence matrix 清理**：审查所有 confidence 值的合理性

或转入 Priority 3/4 维护模式，等待用户路线决策。
