# Closure Review: import binding 多重同符号消歧

日期：2026-05-08
状态：完成
关联 Preflight：`docs/plans/2026-05-08-rust-import-binding-same-symbol-disambiguation-preflight.md`

## Landed Reality

### 修改内容

在 `crates/project-model/src/calls.rs` 的 `resolve_free_function()` 中：

当同模块内存在多个 import binding 指向同一名称时，检查所有已解析 binding 是否指向同一 symbol_id：
- 若全部指向同一 symbol → 解析（confidence 0.85，reason call-import-resolved）
- 若指向不同 symbol → 保持 ambiguous

修复前：
- `resolve_import_target` 在 references.rs:274 和 references.rs:449 被标记为 `call-target-ambiguous`
- 原因：两个不同函数各自 import 了 `resolve_import_target`，两个 binding 都解析成功，但代码要求恰好 1 个已解析 binding

修复后：
- `resolve_import_target` 全部 3 个调用均 resolved（references.rs:274, references.rs:449, graph.rs:679）
- 均通过 import-resolved 路径解析到 `crate::extractors::imports::resolve_import_target`

### 效果

- `resolve_import_target`：2 unresolved → 2 resolved
- FreeFunction unresolved: 16 → 14 (-2)
- call-target-ambiguous（非 method-call）: 3 → 1（剩余 is_tree_sitter_available，cfg-gated stop-line）

### 验证结果

- `cargo fmt --check`: ✅ clean
- `git diff --check`: ✅ clean
- `cargo test`（no-feature）: ✅ 全部通过
- `cargo test --features tree-sitter-cangjie --test graph_contract`: ✅ 24/24 pass
- `project_model_graph_contract`: ✅ 37/37 pass
- `project_model_call_expected_compare`: ✅ 7/7 pass

### Stop-line 合规

- No type inference / trait solving ✅
- No macro expansion ✅
- No cfg evaluator ✅
- No external crate resolution ✅
- No new dependencies ✅
- No GitNexus-RC / Tool / live repo modification ✅
