# CALLS large-file maintenance preflight

日期：2026-05-04
类型：preflight（维护性拆分评估）
状态：完成（推荐安全拆分，进入 execution card）

## 1. 背景

`crates/project-model/src/calls.rs` 当前 2161 行（`wc -l` 实测），远超预估 ~500 行。同时承载：

- tree-sitter call extraction
- text fallback call extraction
- CalleeIndex / ImportBindingTable / CallerIndex 三个内部索引结构
- 6 条 resolution strategy（same-module / import-binding / crate-path / self / super / associated-function）
- method-call dispatch（blind name + stdlib trait + receiver type scan）
- external crate classification
- stdlib tables（prelude types / trait methods / type methods / variable type annotation scan）
- diagnostics / fixture policy
- 多条 stop-line

Rust-core AGENTS.md 和 RISK_LEDGER 的 active quality watch 均要求：继续新增 CALLS 策略前，必须先评估能否 behavior-preserving 拆分。

## 2. 职责块清点

按行号划分：

| 行号范围 | 行数 | 职责块 | 独立性 |
|----------|------|--------|--------|
| 1–34 | 34 | Module doc + imports + `CallExtractionResult` | 不拆（入口类型） |
| 35–107 | 73 | `extract_and_resolve_calls` 主入口 | 不拆（主循环+编排） |
| 109–214 | 106 | `CalleeIndex` / `CalleeMatch` + `build_callee_index` + lookup | 中等（依赖 model types） |
| 216–296 | 81 | `ImportBindingTable` / `ImportBinding` + `build_import_binding_table` + lookup | 中等（与 imports.rs SymbolIndex 对称） |
| 297–342 | 46 | `CallerIndex` / `CallerMatch` + `build_caller_index` + lookup | 低（小，紧耦合） |
| 343–382 | 40 | `extract_calls_from_file` per-file 分发 | 不拆（编排逻辑） |
| 383–530 | 148 | `extract_calls_tree_sitter` + `collect_call_expressions` | 不拆（核心 extraction） |
| 532–718 | 187 | `process_call_expression` + `process_method_call_expression` | 不拆（核心 processing） |
| 719–795 | 77 | `classify_callee` | 不拆（classify 与 process 紧耦合） |
| 796–948 | 153 | `resolve_call_site` 主 resolver dispatcher | 不拆（编排逻辑） |
| 949–1092 | 144 | `resolve_free_function` | 不拆（引用 CalleeIndex / ImportBindingTable） |
| 1093–1198 | 106 | `resolve_qualified_path` | 不拆（引用 CalleeIndex / ImportBindingTable / caller_index） |
| 1199–1276 | 78 | `resolve_self_path` | 不拆（引用 CalleeIndex / caller_index） |
| 1277–1352 | 76 | `resolve_super_path` | 不拆（引用 CalleeIndex / caller_index） |
| 1353–1482 | 130 | `resolve_associated_function` | 不拆（引用 CalleeIndex / ImportBindingTable / resolve_type_module） |
| 1483–1514 | 32 | `resolve_type_module` | 不拆（紧耦合到 associated） |
| 1515–1851 | 337 | **Text fallback 子系统** | **高（独立子系统）** |
| 1852–2161 | 310 | **Stdlib tables + helpers** | **高（纯函数+静态数据）** |

### Text fallback 子系统详细（1515–1851）

- `extract_calls_text_fallback` — text-level entry point
- `parse_text_call` — 单个 call line 解析
- `find_outermost_call` — bracket 匹配
- `classify_text_callee` — text callee 分类
- `resolve_call_site_text` — text call 解析（~140 行）

### Stdlib tables 子系统详细（1852–2161）

- `split_last_segment` — path 分段 helper
- `lookup_prelude_type_path` — 5 个 prelude type 映射
- `lookup_stdlib_trait_method` — 3 个 unique stdlib trait method 映射
- `STDLIB_TYPE_METHODS` — 6 个 stdlib type × method 表（~100 行静态数据）
- `scan_variable_type_annotation` — let 绑定 + 函数参数类型扫描（~100 行）
- `lookup_receiver_type_method` — type + method → path 查找
- `strip_generics` — 泛型参数清除
- `byte_to_line` — byte offset → line number（tree-sitter only）

## 3. 拆分评估

### 3.1 候选 A：Stdlib tables → `stdlib_tables.rs`

| 维度 | 评估 |
|------|------|
| 独立性 | **高**。所有函数都是纯函数，仅依赖 `model::*`（已可访问）和 `stdlib_index`（已 pub）。 |
| 行数 | ~290 行（去掉 `byte_to_line` 11 行不拆，它属于 tree-sitter） |
| 影响面 | calls.rs 新增 `use crate::stdlib_tables::*;`，其他代码不变 |
| 公共 API | 不影响。所有函数当前是 `fn`（non-pub），提取后改为 `pub(crate) fn` |
| CLI output | 不影响 |
| graph schema | 不影响 |
| expected fixtures | 不影响（纯行为等价移动） |
| 风险 | **极低** |

**推荐：拆。**

### 3.2 候选 B：Text fallback → `calls_text.rs`

| 维度 | 评估 |
|------|------|
| 独立性 | **中等**。依赖 `CalleeIndex` / `ImportBindingTable` / `CallerIndex` 的内部 lookup 方法。 |
| 行数 | ~337 行 |
| 影响面 | 需将 `CalleeIndex` / `ImportBindingTable` / `CallerIndex` 的 lookup 方法设为 `pub(crate)`，或将这些结构体也提取。 |
| 公共 API | 不影响 |
| CLI output | 不影响 |
| graph schema | 不影响 |
| expected fixtures | 不影响（纯行为等价移动） |
| 风险 | **低-中**。需暴露内部索引类型，增加模块间耦合面。 |

**推荐：第二刀再拆，本轮先做 stdlib_tables。** 理由：text fallback 依赖三个内部索引结构的 lookup 接口；如果同时拆索引结构，变动面过大。如果只暴露 `pub(crate)` 方法，接口污染可控但不如先积累 stdlib_tables 拆分的经验，确认 module boundary 设计正确后再拆 text fallback。

### 3.3 候选 C：CalleeIndex / ImportBindingTable / CallerIndex 提取

| 维度 | 评估 |
|------|------|
| 独立性 | **低**。三个索引结构被 6 条 resolution strategy 密集引用，也被 text fallback 引用。 |
| 行数 | ~233 行 |
| 影响面 | 大量 resolution strategy 函数需要更新 use 路径 |
| 风险 | **中**。行为等价但变更面大，diff 范围广，review 成本高。 |

**推荐：暂不拆。** 理由：这三个结构是 calls.rs 的内部实现细节，提取后反而增加模块间引用路径的认知负担。当前主要的维护负担来自函数长度和策略混排，而非这三个小结构体。

### 3.4 候选 D：单个 resolution strategy 提取

| 维度 | 评估 |
|------|------|
| 独立性 | **低**。每个 strategy 都依赖 CalleeIndex / ImportBindingTable / CallerIndex，且 strategy 之间有调用关系（如 `resolve_associated_function` 调用 `resolve_type_module`）。 |
| 风险 | **高**。拆一个 strategy 会导致其他 strategy 也需要拆，形成连锁重构。 |

**推荐：不拆。** resolution strategy 之间的内部耦合是正常的；拆散后反而增加跨模块跳转成本。

## 4. 推荐方案

### 本轮第一刀：提取 `stdlib_tables.rs`

- 从 calls.rs 提取 **~290 行** 到 `crates/project-model/src/stdlib_tables.rs`
- calls.rs 从 2161 行 → ~1871 行（-13.4%）
- 纯行为等价移动，**零语义变更**
- 不改 public API / CLI output / graph schema / expected fixtures
- 推荐文件名：`crates/project-model/src/stdlib_tables.rs`
- `lib.rs` 新增 `pub mod stdlib_tables;`（与现有 `pub mod stdlib_index;` 对称）

### 第二刀（后续，不在本轮）：提取 `calls_text.rs`

- 条件：`calls.rs` 再次超过 2000 行，或新增 CALLS 策略前
- 前置条件：第一刀验证通过后，确认 module boundary 设计模式可复用

## 5. Write set

| 路径 | 操作 | 说明 |
|------|------|------|
| `crates/project-model/src/stdlib_tables.rs` | **新增** | 提取的 stdlib tables + helpers |
| `crates/project-model/src/calls.rs` | **修改** | 删除已提取函数，新增 `use` statement |
| `crates/project-model/src/lib.rs` | **修改** | 新增 `pub mod stdlib_tables;` |

## 6. Forbidden set

| 文件 / 模块 | 理由 |
|-------------|------|
| `model.rs` | 不改数据模型 |
| `graph.rs` | 不改 graph schema / emitter |
| `output.rs` | 不改 CLI output contract |
| `imports.rs` | 不改 import resolution |
| `item.rs` | 不改 symbol extraction |
| `manifest.rs` | 不改 project model |
| `module_path.rs` | 不改 module path |
| `root_resolution.rs` | 不改 root resolution |
| `source.rs` | 不改 source ownership |
| `stdlib_index.rs` | 不改已有 sysroot index |
| `diagnostic.rs` | 不改 diagnostic codes |
| `Cargo.toml` | 不新增/修改依赖 |
| 所有 `fixtures/` 目录 | 不改任何 expected-*.json / golden |
| GitNexus-RC 文件 | 不改 TypeScript adapter / schema / package |

## 7. 验证要求

### 7.1 必须验证

1. `cargo fmt --check` — 格式检查
2. `cargo test` — 全量测试（当前 89/89），零 regression
3. `git diff --check` — 无 whitespace 错误
4. Call fixture comparison — 确认 19/19 call fixtures 零 drift
5. Graph fixture comparison — 确认 4 graph fixtures 零 drift
6. Endpoint integrity smoke — `--include calls --include graph` on c1-same-module

### 7.2 必须证明无行为变化

- 所有 89 个 tests 通过（与提取前完全一致的 pass count）
- 所有 19 个 call fixtures 零 golden drift
- 所有 4 个 graph fixtures 零 golden drift
- CLI smoke 输出一致

## 8. 公共 API / CLI / graph schema 影响

- **Public API**：不影响。所有提取的函数是 `fn`（non-pub），不暴露给外部 consumer。
- **CLI output**：不影响。`project-model inspect` 输出格式不变。
- **Graph schema**：不影响。schema version 仍为 "0.3.0"。
- **Expected fixtures**：不影响。golden files 不改变。
- **GitNexus-RC adapter**：不影响。Rust-core 的 graph JSON 格式不变。

## 9. Stop-line

1. 不碰 model.rs / graph.rs / output.rs / Cargo.toml
2. 不修改任何 resolution strategy 的语义逻辑
3. 不新增/删除/重排任何 call fixture 文件
4. 不修改 expected-*.json
5. 不做 text fallback 提取（留给第二刀）
6. 不做 CalleeIndex / ImportBindingTable / CallerIndex 提取
7. 不在本轮混入新的 CALLS resolution 规则
8. 不新增 anyhow / thiserror / 新依赖
9. 不新增 diagnostic code
10. 不做 `pub` 导出（仅 `pub(crate)`）

## 10. 下一步

**推荐进入 execution card。** 本轮只拆 stdlib_tables.rs，范围小、风险极低、行为等价可证明。

## 11. Comment Policy

本轮为纯移动重构，不新增语义边界或维护性注释。函数文档注释原样保留。stdlib_tables.rs 顶部新增一行 module doc 说明来源。
