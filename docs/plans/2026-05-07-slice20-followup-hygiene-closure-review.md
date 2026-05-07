# Slice 20 Follow-up Hygiene — Closure Review

日期：2026-05-07

## 封账声明

Slice 20 follow-up hygiene **已完成**。

## 修复内容

### multi_project_smoke 可移植性

- 问题：`test_multi_project_smoke_with_details` 写死 4 个本机绝对路径（`/Users/jiangxuanyang/Desktop/...`），默认 `cargo test --features tree-sitter-cangjie` 在其他机器上会失败
- 修复：加 `#[ignore]`，默认测试跳过
- 手动 production smoke 命令：`cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`
- 不删路径、不改逻辑、不降级断言

### docs/plans/README.md header

- 更新 "最后更新" 为 Slice 20 follow-up / 264 tests
- 添加 Slice 20 follow-up 已完成条目

## 不改

- 不改 GitNexus-RC runtime/schema/package/web
- 不改 GitNexus-RC-Tool
- 不改 cangjie live repo / index checkout
- 不做 destructive git 操作
- 不新增依赖
- 不开启 Slice 21 implementation
- 不重做 Slice 19/20

## 下一步

可进入 Slice 21 preflight。
