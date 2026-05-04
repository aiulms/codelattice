# CALLS stdlib_tables extraction — execution card

日期：2026-05-04
类型：execution card
来源 preflight：[2026-05-04-calls-large-file-maintenance-preflight.md](2026-05-04-calls-large-file-maintenance-preflight.md)

## Authority

GitNexus-RC `TASK_TRACKER.md` 当前 next opening：CALLS large-file maintenance
Rust-core `AGENTS.md` active quality watch：`calls.rs ~2053 行`
Rust-core `docs/plans/README.md` 推荐下一篇计划：CALLS large-file maintenance preflight

## Goal

从 `calls.rs` 提取 ~290 行纯函数 + 静态数据到新文件 `stdlib_tables.rs`，behavior-preserving。calls.rs 从 2161 行降至 ~1871 行（-13.4%）。

## Allowed Write Set

| 路径 | 操作 | 上限 |
|------|------|------|
| `crates/project-model/src/stdlib_tables.rs` | **新增** | ~300 行 |
| `crates/project-model/src/calls.rs` | **修改** | 删除 7 个 fn + 1 个 static + 1 个 struct；新增 1 行 `use` |
| `crates/project-model/src/lib.rs` | **修改** | 新增 1 行 `pub mod stdlib_tables;` |

## Forbidden Files / Modules

| 类别 | 文件 |
|------|------|
| Runtime logic | `model.rs`, `graph.rs`, `output.rs`, `imports.rs`, `item.rs`, `manifest.rs`, `module_path.rs`, `root_resolution.rs`, `source.rs`, `stdlib_index.rs`, `diagnostic.rs` |
| Config | `Cargo.toml`, `Cargo.lock` |
| Fixtures | 所有 `fixtures/**/expected-*.json` |
| GitNexus-RC | 所有文件 |

## Owner / Truth

- Owner：`crates/project-model/src/calls.rs`（CallSite extraction + resolution）
- Truth：Rust-core test suite（89/89）+ 19 call fixtures + 4 graph fixtures

## Invariants

1. 所有 89 个 cargo tests 通过（pass count 不变）
2. 所有 19 个 call fixtures expected-calls.json 零 gtolden drift
3. 所有 4 个 graph fixtures expected-graph.json 零 golden drift
4. CLI smoke（`--include calls --include graph`）输出 collector_count / call_list / stats 不变
5. `cargo fmt --check` clean
6. `git diff --check` clean
7. 不新增/删除/重排任何 symbol / import / call / graph 边

## Moved Items

从 `calls.rs`（行 1852–2150）移动到 `stdlib_tables.rs`：

| 原行号 | 符号 | 类型 | 可见性 |
|--------|------|------|--------|
| 1852–1857 | `split_last_segment` | fn | pub(crate) |
| 1861–1870 | `lookup_prelude_type_path` | fn | pub(crate) |
| 1876–1884 | `lookup_stdlib_trait_method` | fn | pub(crate) |
| 1893–1900 | `StdlibTypeMethodEntry` | struct | pub(crate) |
| 1904–1997 | `STDLIB_TYPE_METHODS` | static | pub(crate) |
| 2002–2097 | `scan_variable_type_annotation` | fn | pub(crate) |
| 2100–2120 | `lookup_receiver_type_method` | fn | pub(crate) |
| 2126–2150 | `strip_generics` | fn | pub(crate) |

不移动：
- `byte_to_line`（行 2153–2161）：属于 tree-sitter extraction，保留在 calls.rs

## Verification Commands

```bash
cd /Users/jiangxuanyang/Desktop/gitnexus-rust-core
cargo fmt --check
cargo test
git diff --check
cargo run -q -p gitnexus-rust-core-cli -- project-model inspect \
  --root fixtures/call-resolution/c1-same-module --include calls --include graph
```

## Stop-Line

1. 不碰 model.rs / graph.rs / output.rs / Cargo.toml
2. 不修改任何 resolution strategy 的语义逻辑
3. 不新增/删除/重排 fixture 文件
4. 不修改 expected-*.json
5. 不做 text fallback / CalleeIndex / ImportBindingTable / CallerIndex 提取
6. 不在本轮混入新的 CALLS resolution 规则
7. 不新增 anyhow / thiserror / 新依赖
8. 不新增 diagnostic code
9. 不做 `pub` 导出（仅 `pub(crate)`）
10. `byte_to_line` 保留在 calls.rs

## Comment Policy

本轮为纯移动重构，不新增语义边界。保留所有现有文档注释。stdlib_tables.rs 顶部 module doc 注明来源。

## Closure Requirements

- 记录 old/new file layout（calls.rs 前行数 → 新行数）
- 记录验证结果（cargo test pass count、fixture golden drift）
- 记录 residual risk（后续触发条件）
- 更新 Rust-core AGENTS.md quality watch 行数
- 更新 Rust-core RISK_LEDGER.md 行数记录
