# CALLS stdlib_tables extraction — closure review

日期：2026-05-04
类型：implementation closure review
来源 execution card：[2026-05-04-calls-stdlib-tables-extraction-execution-card.md](2026-05-04-calls-stdlib-tables-extraction-execution-card.md)

## 1. Landed Reality

### 1.1 File layout 变化

| 文件 | 操作 | 前行数 | 新行数 | 变化 |
|------|------|--------|--------|------|
| `calls.rs` | 修改 | 2161 | 1858 | -303 (-14.0%) |
| `stdlib_tables.rs` | 新增 | — | 311 | +311 |
| `lib.rs` | 修改 | 19 | 20 | +1 |

### 1.2 提取内容

从 `calls.rs` 提取 8 个符号到 `stdlib_tables.rs`：

| 符号 | 新可见性 | 原行号 |
|------|----------|--------|
| `split_last_segment` | `pub(crate)` | 1852–1857 |
| `lookup_prelude_type_path` | `pub(crate)` | 1861–1870 |
| `lookup_stdlib_trait_method` | `pub(crate)` | 1876–1884 |
| `StdlibTypeMethodEntry` | `pub(crate)` | 1893–1900 |
| `STDLIB_TYPE_METHODS` | `pub(crate)` | 1904–1997 |
| `scan_variable_type_annotation` | `pub(crate)` | 2002–2097 |
| `lookup_receiver_type_method` | `pub(crate)` | 2100–2120 |
| `strip_generics` | `pub(crate)` | 2126–2150 |

保留在 `calls.rs`：
- `byte_to_line`（原 2153–2161）：tree-sitter only，与 extraction 无关

### 1.3 未提取内容

按 preflight 建议，本轮**未提取**：
- Text fallback 子系统（~337 行）：依赖 CalleeIndex / ImportBindingTable / CallerIndex 内部接口
- CalleeIndex / ImportBindingTable / CallerIndex（~233 行）：与 6 条 resolution strategy 紧耦合
- 单条 resolution strategy：strategy 之间有内部调用链

这些留待后续或第二刀。

## 2. Invariants

| Invariant | 状态 |
|-----------|------|
| 所有 89 个 cargo tests 通过 | ✅ 89/89 pass |
| 19 个 call fixtures 零 golden drift | ✅ call_comparison_passes_for_all_fixtures pass |
| 4 个 graph fixtures 零 golden drift | ✅ graph_comparison_passes_for_all_fixtures pass |
| 5 个 import fixtures 零 golden drift | ✅ import_comparison_passes_for_all_fixtures pass |
| 4 个 symbol fixtures 零 golden drift | ✅ symbol_comparison_passes_for_all_fixtures pass |
| CLI smoke 输出正常 | ✅ `--include calls --include graph` 输出 schemaVersion "0.3.0" |
| `cargo fmt --check` clean | ✅ |
| `git diff --check` clean | ✅ |
| 不新增/删除/重排 fixture 文件 | ✅ |
| 不新增/删除/重排 graph edge | ✅ |

## 3. 验证结果

```bash
cargo fmt --check     # ✅ PASS
cargo test            # ✅ 89/89 PASS (0 failures)
git diff --check      # ✅ PASS
CLI smoke             # ✅ valid JSON, schemaVersion 0.3.0
```

Test 明细：
- 4 stdlib_index tests
- 7 call comparison tests
- 10 project model comparison tests
- 10 graph emit tests
- 4 graph comparison tests
- 5 import comparison tests
- 45 inspect tests
- 4 symbol comparison tests

## 4. Public API / CLI / Graph Schema 影响

- **Public API**：不影响。所有提取符号为 `pub(crate)`。
- **CLI output**：不影响。`project-model inspect` 输出不变。
- **Graph schema**：不影响。schema version 仍为 "0.3.0"。
- **Expected fixtures**：不影响。零 golden drift。
- **GitNexus-RC adapter**：不影响。graph JSON 格式不变。

## 5. Comment Policy

本轮为纯移动重构，未新增语义边界。所有函数文档注释原样保留。stdlib_tables.rs 顶部新增 4 行中文 module doc 说明来源和内容。

## 6. Stop-Line

| Stop-Line | 守住 |
|-----------|------|
| 不碰 model.rs / graph.rs / output.rs / Cargo.toml | ✅ |
| 不修改 resolution strategy 语义逻辑 | ✅ |
| 不新增/删除/重排 fixture | ✅ |
| 不修改 expected-*.json | ✅ |
| 不做 text fallback / CalleeIndex / ImportBindingTable / CallerIndex 提取 | ✅ |
| 不混入新的 CALLS resolution 规则 | ✅ |
| 不新增依赖 | ✅ |
| 不新增 diagnostic code | ✅ |
| 不做 `pub` 导出（仅 `pub(crate)`） | ✅ |
| `byte_to_line` 保留在 calls.rs | ✅ |

## 7. Residual Risk

| 风险 | 级别 | 说明 |
|------|------|------|
| calls.rs 仍 1858 行 | LOW | 已从 2161 降至 1858，但仍在 2000 线附近。继续新增 CALLS 策略前需再次评估。 |
| Text fallback 未提取 | LOW | 337 行独立子系统留待第二刀；当前不影响功能维护。 |
| CalleeIndex / ImportBindingTable / CallerIndex 数据冗余 | LOW | 已有记录但暂不拆；与 resolution strategy 耦合紧密。 |
| 第一次拆分 module boundary | LOW | stdlib_tables 是 Rust-core project-model 第一个从 calls.rs 拆出的模块，设计模式已验证可行。 |

## 8. 下一次触发条件

当以下任一条件满足时，触发第二刀（text fallback 提取或进一步拆分）：
- `calls.rs` 再次超过 2000 行
- 需要新增第 8 条 resolution strategy
- 需要新增新的 call kind

## 9. 不能自动重开的线

- 不把已提取的 stdlib_tables.rs 内容 merge 回 calls.rs
- 不在 text fallback / CalleeIndex 提取前新增 CALLS 策略
- 不把 `pub(crate)` 升为 `pub`
