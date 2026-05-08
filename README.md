# GitNexus Rust-core

> **Remote:** https://gitcode.com/aiulms/gitnexus-rust-core
> **Branch:** `master`
> **Created:** 2026-05-01
> **Last updated:** 2026-05-09 (Productization Phase complete)

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
| 4. CALLS Intermediate | Call site extraction + 12 resolved call forms + method dispatch + stdlib trait + external crate path + enum constructor + cross-file same-crate + wildcard disambiguation | ✅ Implemented | 22 call fixtures |
| 5. Graph Emitter v0.3 | ProjectModel → JSON graph (CALLS + DESIGNATION + ACCESSES edges) + external symbol node completion | ✅ Implemented | 6 graph fixtures + portable-smoke |

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
| Method dispatch (constructor chain) | `v.push(1)` where `let v = Vec::new()` | 0.65 |
| Stdlib trait method | `x.to_string()`, `y.clone()` | 0.55 |
| External crate path | `Vec::new()`, `String::from()` | 0.80–0.85 |
| Enum constructor | `Some(42)`, `Ok(val)`, `Err(e)` | 0.80 |
| Cross-file same-crate (Phase 2e) | `split_last_segment()` from another module | 0.80 |
| Wildcard-aware disambiguation (Phase 2f) | `helper_func()` via `use calculations::*` | 0.80 |

**Resolution rate: 65.8%**（2339/3557 calls on gitnexus-rust-core，2026-05-08 Phase 2g associated-function disambiguation）。

---

## CLI Usage

### Rust Project Analysis

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

### Cangjie Project Analysis (Rust-native Local Trial)

**Note:** Cangjie CLI commands require the `tree-sitter-cangjie` feature to be enabled. This is a **local trial implementation** and is not intended to replace the production GitNexus-RC tool.

```bash
# Enable Cangjie feature
cargo build --features tree-sitter-cangjie -p gitnexus-rust-core-cli

# Inspect Cangjie project (outputs graph JSON)
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- cangjie inspect \
  --root /path/to/cangjie/project

# Output Cangjie graph (same as inspect)
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- cangjie graph \
  --root /path/to/cangjie/project
```

**Feature Requirements:**
- `--features tree-sitter-cangjie` must be specified when building and running
- The `tree-sitter-cangjie` feature enables Cangjie language support via tree-sitter parser
- Without this feature, the `cangjie` subcommand is not available

**Current Capabilities (Slice 17):**
- Project model scanning (cjpm.toml, workspace members, source files)
- Symbol extraction (Function, Class, Struct, Enum, Interface, TypeAlias, Macro)
- Import resolution (named imports, path dependencies, cjpm tree external deps)
- Reference extraction (same-file and cross-file type annotations)
- Graph output (Repository/Package/SourceFile/Symbol nodes + edges)
- Deterministic JSON output to stdout

**Stop-lines for Cangjie:**
- No full method dispatch (blind name + stdlib trait + receiver type heuristics only)
- No type inference / trait solving
- No macro expansion
- No full cfg evaluator
- No production replacement for GitNexus-RC tool

### Unified Productization CLI（本地试用入口）

面向产品化的统一 CLI 入口，提供 analyze / quality / summary 三个命令和语言自动检测。

```bash
# analyze — 完整分析（graph + quality gates）
cargo run -p gitnexus-rust-core-cli -- analyze --root fixtures/rust/portable-smoke --format json
cargo run -p gitnexus-rust-core-cli -- analyze --root . --language auto --format json

# quality — 质量门检查（exit code: 0=pass, 1=fail, 2=ambiguous）
cargo run -p gitnexus-rust-core-cli -- quality --root fixtures/rust/portable-smoke --language rust
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  quality --root fixtures/cangjie/portable-smoke --language cangjie

# summary — 统计摘要（不含完整 graph）
cargo run -p gitnexus-rust-core-cli -- summary --root fixtures/rust/portable-smoke --language rust --format json
cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- \
  summary --root fixtures/cangjie/portable-smoke --language cangjie --format json
```

**语言自动检测（--language auto）：**
- 存在 `Cargo.toml` → `rust`
- 存在 `cjpm.toml` → `cangjie`
- 两者都存在 → 报错，要求显式指定

**Bridge 格式（--format gitnexus-rc）：**
- 将 Rust/Cangjie graph 转换为 GitNexus-RC 兼容格式
- 归一化 edge 端点字段（source/target → sourceId/targetId）
- 节点按 kind 显式分类，边按类型分组
- Stop-line：不修改 GitNexus-RC 代码，不做 production replacement

---

## Verification

```bash
cargo fmt --check                                     # Formatting check
cargo test                                            # 200+ tests (no-feature)
cargo test --features tree-sitter-cangjie             # 330+ tests (with feature)
cargo test --features tree-sitter-cangjie \
  --test cangjie_inspect -- --nocapture               # Cangjie CLI tests (18)
cargo test --features tree-sitter-cangjie \
  --test graph_contract -- --nocapture                # Cangjie contract regression (24)
cargo test --features tree-sitter-cangjie \
  --test multi_project_smoke -- --nocapture           # Cangjie fixture smoke (4)
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
      src/
        main.rs                          # CLI entry point
        bridge_format.rs                 # GitNexus-RC 兼容格式 adapter（NEW）
        unified_types.rs                 # 统一输出类型定义（NEW）
        language_detect.rs               # 语言自动检测（NEW）
      tests/
        project_model_inspect.rs         # Integration tests
        productization_commands.rs        # Productization CLI tests (15)（NEW）
  fixtures/
    manifest-scanner/                      # 6 fixtures
    root-resolution/                       # 9 fixtures
    source-ownership/                      # 8 fixtures
    item-extraction/                       # 10 fixtures (with expected-symbols.json)
    import-use/                            # 12 fixtures (with expected-imports.json)
    call-resolution/                       # 22 fixtures (C1-C14 + SF1-SF6 + call-enum-filter + call-module-path, with expected-calls.json)
  docs/
    architecture/                          # Architecture docs (unified-output-contract, bridge-preflight)
    decisions/                             # Decision records
    fixtures/                              # Fixture index
    migration/                             # Migration from GitNexus-RC
    plans/                                 # Preflight / execution card / closure review
    smoke-targets-config.md                # Smoke targets list (NEW)
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
| HEAD | `5363eb8` |
| Total commits | 150 |

---

## License

本项目遵循 GitNexus PolyForm Noncommercial 许可证。参见 GitNexus-RC LICENSE。
