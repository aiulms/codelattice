# Closure Review: impl 块泛型目标类型解析修复

日期：2026-05-08
状态：完成
关联 Preflight：`docs/plans/2026-05-08-rust-impl-generic-target-parsing-preflight.md`

## Landed Reality

### 修改内容

在 `crates/project-model/src/item.rs` 的 `parse_impl_header()` 中：

将 `generic_type` 和 `scoped_type_identifier` 从跳过改为递归查找子节点中的 `type_identifier`。

修复前：
- `impl<'a> SameFileIndex<'a>` → `SameFileIndex<'a>` 被解析为 `generic_type` 节点 → 跳过 → impl_target 未找到 → fallback `"Unknown"`
- `_impl_Unknown` 符号 → SameFileIndex::build 无法通过 impl_target 过滤解析

修复后：
- `generic_type` 子节点递归查找 `type_identifier` → 找到 "SameFileIndex"
- `_impl_SameFileIndex` 符号 ✅

### 效果

- `SameFileIndex::build()` ×5：全部从 unresolved → resolved (+5)
- `associated-function resolved`: 2 → 7 (+5)
- `unresolved associated-function`: 15 → 10 (-5)
- Resolution rate: 65.8% → 65.9%（2344→2352/3571）

### 验证结果

- `cargo fmt --check`: ✅ clean
- `git diff --check`: ✅ clean
- `cargo test`（no-feature）: ✅ 全部通过
- `project_model_graph_contract`: ✅ 37/37 pass
- `project_model_call_expected_compare`: ✅ 7/7 pass
- Golden fixture 零漂移

### Stop-line 合规

- No type inference / trait solving ✅
- No macro expansion ✅
- No external crate resolution ✅
- No new dependencies ✅
- No GitNexus-RC / Tool / live repo modification ✅
