# C Phase A — 遗留 C 代码图谱 / MCP 支持

Date: 2026-05-15
Status: **Phase A shipped** (stages 1-9 complete)

## Goal

Add C language Phase A support to CodeLattice: new `gitnexus-c` crate, tree-sitter-c extractor, C fixture, CLI `analyze/quality/summary --language c`, MCP `language=c` support, tests, scripts, docs.

## Phase A Scope (Completed)

### Stage 1-5: Crate skeleton
- `crates/c/Cargo.toml`: new `gitnexus-c` package, feature `tree-sitter-c`
- `crates/c/src/lib.rs`: public API exports
- `crates/c/src/extractors/mod.rs`: feature-gated parser init
- `crates/c/src/extractors/symbol.rs`: `CSymbolKind`, `CSymbol`, `CVisibility`, `extract_c_symbols`
- `crates/c/src/extractors/include.rs`: `CIncludeKind`, `CInclude`, `extract_c_includes`
- `crates/c/src/project.rs`: `CProject`, `CProjectKind`, `find_c_project_root`, C++ exclusion
- `crates/c/src/graph.rs`: `CNodeKind`, `CEdgeKind`, `CGraphOutput`, `build_c_graph`
- Compiles clean: `cargo check -p gitnexus-c --features tree-sitter-c` → 0 errors

### Stage 6: CLI + bridge integration
- `crates/cli/src/unified_types.rs`: `DetectedLanguage::C` added
- `crates/cli/src/language_detect.rs`: C detection + C++ exclusion
- `crates/cli/src/lib.rs`: `mod c_bridge`, `run_c_analysis()`, Analyze + Quality + Summary match arms
- `crates/cli/src/c_bridge.rs`: `convert_c_graph` (independent from arkts bridge)
- `crates/cli/src/bridge_format.rs`: `pub use c_bridge::convert_c_graph`
- `crates/c/src/lib.rs`: re-export `extract_c_symbols`, `extract_c_includes` under feature gate
- All `cargo check` passes

### Stage 7: MCP integration
- 21 tool schemas updated: added `"c"` to language enum
- `check_language_feature` updated: `language == "c"` with `#[cfg(not(feature = "tree-sitter-c"))]` guard
- `serverInfo.cSupport` added to MCP initialize response
- `scripts/codelattice-mcp.sh`: `cSupport` profile extraction + all-languages check

### Stage 8: MCP tests (9 feature-gated tests)
- `mcp_c_analyze_portable_smoke`: CLI analyze → JSON
- `mcp_c_quality_portable_smoke`: CLI quality → JSON
- `mcp_c_summary_portable_smoke`: CLI summary → JSON
- `mcp_c_bridge_format`: CLI bridge → gitnexus-rc format
- `mcp_c_symbol_search_finds_add`: MCP symbol_search
- `mcp_c_project_overview_counts_nonzero`: MCP project_overview
- `mcp_c_calls_from_main`: MCP calls_from
- `mcp_c_query_graph_finds_math_utils`: MCP query_graph
- `mcp_c_production_assist`: MCP production_assist
- All 113 tests pass (104 original + 9 C)

### Stage 9: Scripts
- `scripts/c-real-project-smoke.sh`: synthetic + real-project C smoke (4 PASS checks)
- `scripts/mcp-dogfood.sh`: error message fix (>= 21, not >= 24)
- `scripts/codelattice-mcp.sh`: `cSupport` profile + all-languages binary selection

### Documentation
- README.md: C language section added (between Cangjie and ArkTS), auto-detect rule for C
- CHANGELOG.md: C Phase A entry under Added

## Phase A Limitations

- No macro expansion (no `#define` evaluation)
- No function pointer call resolution
- No C++ support (separate adapter, auto-detect returns ambiguous when `.cpp/.cc/.cxx/.hpp/.hh/.hxx` found)
- No build system execution
- Not a replacement for clang / clangd
- Bridge: minimum stable fields only (no package concept for C)
- MCP tools: `calls_to`, `query_graph` nodeKind filter have schema limitations with C graph

## What's NOT included in Phase A

- Full C++ support (separate design)
- Complete macro expansion
- Function pointer analysis
- Compiler-level semantics
- Multi-language mixed projects (C + C++)

## Key Files

```
crates/c/Cargo.toml                          # new package
crates/c/src/lib.rs                         # public API
crates/c/src/extractors/symbol.rs            # symbol extraction
crates/c/src/extractors/include.rs            # include extraction
crates/c/src/graph.rs                        # graph output types
crates/c/src/project.rs                      # project detection + C++ exclusion
crates/cli/src/c_bridge.rs                   # bridge converter (new)
crates/cli/src/unified_types.rs              # DetectedLanguage::C
crates/cli/src/language_detect.rs           # C detection
crates/cli/src/lib.rs                        # CLI commands + run_c_analysis
crates/cli/src/bridge_format.rs              # convert_c_graph re-export
crates/cli/src/mcp_server.rs                 # language enum + check_language_feature + cSupport
crates/cli/tests/mcp_server.rs               # 9 C-specific tests (feature-gated)
fixtures/c/portable-smoke/                   # C fixture (6 files + Makefile)
scripts/c-real-project-smoke.sh             # new smoke script
scripts/codelattice-mcp.sh                  # cSupport profile
```

## Verification

```bash
cargo fmt --check                              # OK
cargo check                                    # 0 errors
cargo check --features tree-sitter-c           # 0 errors
cargo test --test mcp_server                  # 104 passed
cargo test --test mcp_server --features tree-sitter-c  # 113 passed
bash scripts/c-real-project-smoke.sh           # PASS=4 FAIL=0
bash scripts/c-real-project-smoke.sh --project fixtures/c/portable-smoke  # PASS=4 FAIL=0
```

## Next Steps (Phase B — not in scope)

- C++ support (separate adapter, separate feature flag)
- Function pointer call resolution
- Macro expansion
- Multi-language project handling
- Complete C++ exclusion detection (auto-detect ambiguous)
