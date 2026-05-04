# GitNexus Rust-core

> **Remote:** https://gitcode.com/aiulms/gitnexus-rust-core
> **Branch:** `master`
> **Created:** 2026-05-01
> **Last updated:** 2026-05-04

---

## Purpose

GitNexus Rust-core 是 GitNexus 项目的 Rust 语言分析核心实现。它不是 GitNexus-RC（TypeScript 主仓库）的替代发布版，而是独立的 Rust 工具链，提供 Cargo 项目扫描、符号提取、import 解析、call graph 中间输出和图发射能力。

**与 GitNexus-RC 的关系：**
- GitNexus-RC 是治理来源、架构决策记录和 TypeScript adapter 主仓库
- Rust-core 是 Rust 实现主体，产出可被 GitNexus-RC experimental adapter 消费的 JSON artifacts
- 所有语言支持决策、fixture 设计、confidence/reason 策略源自 GitNexus-RC `docs/language-support/`

---

## Current Capabilities

| Layer | Capability | Status | Fixtures |
|-------|-----------|--------|----------|
| 1. ProjectModel | Cargo manifest scan + workspace + target resolution | ✅ Implemented | 14 PM fixtures |
| 2. Symbol Extraction | tree-sitter + text-level, 10+ symbol kinds | ✅ Implemented | 10 symbol fixtures |
| 3. Import Resolution | `use` declarations + module-level + symbol-level | ✅ Implemented | 12 import fixtures |
| 4. CALLS Intermediate | Call site extraction + 9 resolved call forms + method dispatch + stdlib trait + external crate path | ✅ Implemented | 19 call fixtures |
| 5. Graph Emitter v0.3 | ProjectModel → JSON graph (CALLS + DESIGNATION + ACCESSES edges) | ✅ Implemented | 5 graph fixtures |

### CALLS Resolved Call Forms

| Call Form | Example | Confidence |
|-----------|---------|-----------|
| Same-module free function | `helper()` | 0.90 |
| Import-resolved binding | `use crate::math::add; add()` | 0.85 |
| crate:: qualified path | `crate::math::add()` | 0.90 |
| self:: path | `self::inner_helper()` | 0.80 |
| super:: path | `super::parent_fn()` | 0.80 |
| Associated function | `Config::new()` | 0.75 |
| Same-file unique-name | `helper()` (heuristic) | 0.70 |
| Bare module path | `math::add()` | 0.85 |
| Method dispatch (blind name) | `obj.increment()` | 0.65 |
| Method dispatch (receiver type) | `v.push(1)` where `let v: Vec<i32>` | 0.65 |
| Stdlib trait method | `x.to_string()`, `y.clone()` | 0.55 |
| External crate path | `Vec::new()`, `String::from()` | 0.80–0.85 |

**Resolution rate: 54.0%** (1189/2203 calls on gitnexus-rust-core, v4 consolidation 2026-05-04).

---

## CLI Usage

```bash
# Full project model inspection
cargo run -p gitnexus-rust-core-cli -- project-model inspect \
  --root /path/to/rust/project \
  --format json

# Include specific outputs
cargo run -p gitnexus-rust-core-cli -- project-model inspect \
  --root /path/to/rust/project \
  --format json \
  --include symbols \
  --include imports \
  --include calls \
  --include graph

# Graph output only
cargo run -p gitnexus-rust-core-cli -- project-model inspect \
  --root /path/to/rust/project \
  --format json \
  --include graph
```

`--include calls` automatically triggers `--include symbols` and `--include imports` internally.

---

## Verification

```bash
cargo fmt --check    # Formatting check
cargo test           # 89 tests (7 call + 10 PM + 10 graph + 4 symbol + 5 import + 45 unit + 4 harness + 4 stdlib_index)
```

---

## Directory Structure

```
gitnexus-rust-core/
  Cargo.toml                              # Cargo workspace root
  crates/
    project-model/                         # Core analysis library
      src/
        lib.rs                             # Library root
        model.rs                           # Data models (Symbol, ImportUse, CallSite, etc.)
        item.rs                            # Symbol extraction (tree-sitter + text)
        imports.rs                         # Import resolution
        calls.rs                           # CALLS intermediate output (1858 lines)
        stdlib_tables.rs                   # Stdlib lookup tables (prelude types, trait methods, type methods)
        stdlib_index.rs                    # Static stdlib symbol index (~90 entries)
        graph.rs                           # Graph emitter v0
        module_path.rs                     # ModulePathMap
        manifest.rs                        # Cargo.toml scanner
        root_resolution.rs                 # Root resolution
        source_ownership.rs                # Source ownership
        output.rs                          # CLI output formatting
      tests/
        project_model_expected_compare.rs  # PM comparison harness
        project_model_symbol_expected_compare.rs  # Symbol comparison
        project_model_import_expected_compare.rs   # Import comparison
        project_model_call_expected_compare.rs     # Call comparison
        project_model_graph_expected_compare.rs    # Graph comparison
    cli/
      src/main.rs                          # CLI entry point
      tests/
        project_model_inspect.rs           # Integration tests
  fixtures/
    manifest-scanner/                      # 6 fixtures
    root-resolution/                       # 9 fixtures
    source-ownership/                      # 8 fixtures
    item-extraction/                       # 10 fixtures (with expected-symbols.json)
    import-use/                            # 12 fixtures (with expected-imports.json)
    call-resolution/                       # 15 fixtures (C1-C7 + SF1-SF6 + call-enum-filter + call-module-path, with expected-calls.json)
  docs/
    architecture/                          # Architecture docs
    decisions/                             # Decision records
    fixtures/                              # Fixture index
    migration/                             # Migration from GitNexus-RC
```

---

## Stop-lines (MVP)

以下内容是 Rust-core MVP 的明确 stop-line：

- **No production replacement** — Rust-core 不是 GitNexus-RC TypeScript adapter 的替代
- **No full method dispatch** — `obj.method()` 支持 blind name + stdlib trait + receiver type heuristics，但不做完整 receiver type inference（stop-line）
- **No type inference / trait solving** — 不推断变量类型，不做 trait bound satisfaction
- **No arbitrary external crate resolution** — 仅支持 std/core/alloc direct path（Phase 1），第三方 crate API 不解析（stop-line）
- **No macro expansion** — `foo!()` 不展开
- **No full cfg evaluator** — cfg-gated `mod` 只标记 `unknown`
- **No `cargo metadata` execution`** — 只用 manifest-derived project model
- **No proc-macro / build.rs** — 不执行
- **No UI / MCP server / commercial distribution**

---

## Remote

| Property | Value |
|----------|-------|
| Remote name | `gitcode` |
| URL | `https://gitcode.com/aiulms/gitnexus-rust-core.git` |
| Branch | `master` |
| HEAD | `41e0884` |
| Total commits | 42 |

---

## License

本项目遵循 GitNexus PolyForm Noncommercial 许可证。参见 GitNexus-RC LICENSE。
