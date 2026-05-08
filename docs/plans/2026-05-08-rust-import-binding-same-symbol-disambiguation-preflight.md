# Preflight: import binding 多重同符号消歧

日期：2026-05-08
状态：Preflight
优先级：Priority 1 — Rust CALLS resolution quality

## 问题描述

`resolve_free_function()` 的 import binding 查找（step 2）在遇到同模块内多个 import binding 时，
即使所有 binding 都解析到同一个 symbol，也会判为 `call-target-ambiguous`。

具体案例：`references.rs` 中两个不同函数各自 import 了 `resolve_import_target`：
- Line 259: `use super::imports::{resolve_import_target, ImportCandidate};`
- Line 372: `use super::imports::{parse_named_import_candidates, resolve_import_target};`

两者都解析到 `crate::extractors::imports::resolve_import_target`，但当前代码要求
`resolved.len() == 1`（恰好一个已解析的 binding），导致两个 binding 都解析成功时被误判为歧义。

## 根因

`resolve_free_function()` 第 1152-1178 行：

```rust
multiple if !multiple.is_empty() => {
    let resolved: Vec<_> = multiple
        .iter()
        .filter(|b| b.resolved_symbol_id.is_some())
        .collect();
    if resolved.len() == 1 {
        // resolve
    } else {
        // ambiguous ← 当所有 binding 都指向同一 symbol 时仍然触发
    }
}
```

条件 `resolved.len() == 1` 过于严格：它要求恰好一个 binding 解析成功。
应该改为：检查所有已解析 binding 是否指向同一个 symbol_id。

## 修复

在 `resolved.len() > 1` 时，检查所有已解析 binding 是否指向同一个 symbol_id：
- 若指向同一 symbol → 解析（confidence 0.85，reason call-import-resolved）
- 若指向不同 symbol → 保持 ambiguous

同时优化：当 `resolved.len() == 0` 时，已有 binding 但全部未解析到 symbol 时，
应 fall through 到 step 2.5（cross-file same-crate 搜索）而非直接标记为 unresolved。

## Write Set

- `crates/project-model/src/calls.rs`：修改 `resolve_free_function` 的多 binding 处理

## Forbidden Set

- 不修改 GitNexus-RC / Tool / live repo
- 不新增依赖
- 不修改 CalleeIndex / ImportBindingTable 数据结构
- 不修改 Cangjie 代码

## Acceptance Criteria

1. `resolve_import_target` 2 个调用从 unresolved → resolved
2. 所有现有测试通过（no-feature + feature-enabled）
3. graph contract 37/37 不变
4. `cargo fmt --check` + `git diff --check` clean

## Stop-line Check

- No type inference / trait solving ✅
- No macro expansion ✅
- No cfg evaluator ✅
- No external crate resolution ✅
- No destructive git ✅
