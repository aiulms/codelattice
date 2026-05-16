# CodeLattice Changelog

All notable CodeLattice release changes are tracked here.

This project follows the release policy in `docs/release-versioning.md`. The product version comes from Cargo `workspace.package.version`; MCP `serverVersion` is a separate runtime/tool-profile version.

## [Unreleased]

### Added

- **Entry Point & Reachability Map** (v0.20): New MCP tool `codelattice_reachability_map` — multi-language entry point detection + static BFS reachability analysis.
  - Entry point detection heuristics for Rust, Python, TypeScript/ArkTS, C, C++, and Cangjie (main, lib.rs, index.ts, __init__.py, etc.), with confidence scores and reason tags.
  - BFS reachability traversal from detected entry points (configurable max depth 1–32, default 8), following CALLS/REFERENCES/IMPORTS/INCLUDES/DEFINES edges.
  - Unreachable candidate classification: symbols not reachable from any entry point, with cautions for public API exposure and dynamic dispatch patterns.
  - Compact mode for AI-agent-friendly output (omits verbose IDs).
  - All output includes `generatedFrom: { staticAnalysisOnly: true, heuristic: true, compilerVerified: false }` — no proof/guarantee/deletion-safe language.
  - Reachability summary integrated into `codelattice_dead_code_candidates`, `codelattice_project_insights` (release_check mode), and `codelattice_review_plan`.
  - New fixture: `fixtures/typescript/reachability-map/` (9 files with layered architecture, public API surface, dynamic dispatch patterns).
  - 10 new MCP integration tests.
  - MCP tool count: 30 → 31.
  - No new dependencies.

- **Graph Diagnostics Pack** (v0.19): 5 new MCP tools that package existing static graph capabilities into user-scenario-oriented diagnostic tools, turning CodeLattice from a "code graph analyzer" into a "local code diagnostics engine."
  - `codelattice_impact_analysis` — Change impact analysis: find direct callers/callees, upstream/downstream paths, entry point reachability, risk scoring, read-first and review-first recommendations.
  - `codelattice_risk_hotspots` — Project risk hotspot detection: identify high fan-in/fan-out symbols and files, cross-module dependencies, public API exposure, and quality metric anomalies.
  - `codelattice_architecture_drift` — Architecture health analysis: detect cycle candidates, cross-layer calls (with user-provided layer rules), boundary leaks, and overly coupled modules.
  - `codelattice_ai_context_pack` — AI editing context: given a task description or target symbols, output relevant files, key symbols, call chains, suggested read order, and do-not-assume warnings — ready to feed directly into AI assistants.
  - `codelattice_review_gate` — Diff-based review gate: analyze git diff or specified changed files for touched symbols, hotspot exposure, impact summary, and review checklist.
  - All 5 tools follow unified output contract: `generatedFrom.staticAnalysisOnly=true`, `heuristic=true`, `compilerVerified=false`. No proof/guarantee/deletion-safe language.
  - New fixture: `fixtures/typescript/graph-diagnostics/` (8 files with layered architecture: api → service → domain → infra, cycle candidates, test file).
  - 20 new MCP integration tests.
  - MCP tool count: 25 → 30.
  - No new dependencies.

- **Dead Code Candidate Analysis** (v0.18): New MCP tool `codelattice_dead_code_candidates` identifies statically unreachable symbols and files via graph-based reachability analysis.
  - Scoring algorithm: per-symbol and per-file candidate scoring with positive signals (no incoming edges, not reachable from entry points, private visibility) and negative signals (public/exported, entry-like name, dynamic patterns).
  - Confidence tiers: high (>=0.80), medium (>=0.55), low (<0.55). Candidates below 0.45 are excluded.
  - Entry point detection: language-specific heuristics for main/lib.rs/index.ts etc., user-provided `entryHints`, BFS reachability traversal (max depth 8).
  - Public API cautions: exported/public symbols get confidence capped at "medium" with `public-api-may-have-external-callers` caution.
  - Dynamic feature cautions: registry/plugin/route patterns get `dynamic-dispatch-may-hide-callers` caution and -0.15 score penalty.
  - All output explicitly states `deletionSafe: false` and includes `static-analysis-only` caution — never claims deletion proof.
  - New fixture: `fixtures/typescript/dead-code-candidates/` (7 files covering live, legacy, public-api, dynamic, and test scenarios).
  - 9 new MCP integration tests (feature-gated behind `tree-sitter-typescript`).
  - MCP tool count: 24 → 25.
  - No new dependencies.

## [0.14.0-beta.1] - 2026-05-16

### Added

- **Unified quality metrics** across MCP outputs: `qualityMetrics` is now available in project overview, project insights, review plan release checks, and production assist.
- **Real-project corpus baseline** for beta validation, covering Redis (C), Catch2 (C++), and pip (Python) as the default non-vendored smoke/baseline set.
- **TypeScript path alias and monorepo import resolution**: tsconfig `baseUrl` / `paths`, `extends`, extensionless imports, index resolution, and workspace package imports now resolve to real files where possible.
- **Python import resolution refinement**: package-aware module index, src-layout / flat-layout detection, relative imports, parent-relative imports, aliases, and simple `__init__.py` re-export chains.
- **C and C++ compile_commands include resolution**: `-I`, `-iquote`, `-isystem`, forced includes, diagnostics for unresolved/system includes, and no synthetic unresolved include targets.
- **Release beta notes** for `0.14.0-beta.1` and a real corpus baseline report for external beta validation.

### Changed

- README front matter is now product-facing Chinese copy: CodeLattice is presented as a local code intelligence engine for large, legacy, and complex codebases.
- Release artifact and release smoke now target the full seven-language beta set: Rust, Cangjie, ArkTS, TypeScript, C, C++, Python.
- Release manifests now record source commit, product version, MCP serverVersion, language support flags, tool count, and build features.
- MCP/install documentation now uses parameterized stable wrapper paths such as `/path/to/CodeLattice-Tool/codelattice-mcp.sh`.

### Fixed

- Local promotion, install, and release packaging scripts build and validate the full optional language set: Cangjie, ArkTS, TypeScript, C, C++, and Python.
- Default `cargo test` no longer compiles C/C++ graph integration tests that require `tree-sitter-c` / `tree-sitter-cpp` unless those features are enabled. The same graph tests still run under `cargo test --all-features`.

### Breaking Changes

- None.

### Known Limitations

- CodeLattice is not a compiler, IDE, language server, or hosted upload service.
- Rust does not perform full type inference, trait solving, or macro expansion.
- C/C++ do not perform full preprocessing, template instantiation, overload resolution, or virtual dispatch resolution.
- TypeScript does not run `tsc` and does not provide type-system guarantees.
- Python analysis is static and does not execute imports, virtual environments, monkey patches, or dynamic import code.
- Beta users should pin versions and run self-test / release smoke after upgrades.

## [0.13.0-beta.2] - 2026-05-15

### Added

- Release artifact now builds with `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript`, so the published macOS Apple Silicon tarball includes Rust, Cangjie, ArkTS, and TypeScript adapters.
- Packaged release smoke now includes ArkTS and TypeScript portable fixtures and verifies they analyze successfully from the unpacked tarball.
- MCP `initialize.serverInfo` now reports `arktsSupport` and `typescriptSupport` alongside `cangjieSupport`, making packaged language capability drift visible.

### Fixed

- Fixed `v0.13.0-beta.1` packaging drift where README/CHANGELOG advertised ArkTS production-trial support but the published binary was built without `tree-sitter-arkts`.

### Notes

- `v0.13.0-beta.1` remains immutable for checksum integrity. `v0.13.0-beta.2` supersedes it for external beta users who need ArkTS or TypeScript from the prebuilt package.

## [0.13.0-beta.1] - 2026-05-15

### Added

- `feat(mcp)`: add compact AI-sidecar outputs (`a7b1652`) - compact mode for MCP tools returns stripped-down results for AI context efficiency.
- `feat(arkts)`: complete production trial analysis path (`559f44a`) - ArkTS/HarmonyOS analysis works end-to-end via tree-sitter-typescript, component/buildMethod extraction.
- `feat(mcp)`: detect changed symbols from git diff (`9d0b157`) - auto-detect changed symbols from unstaged/staged/all git diff, map hunks to graph symbols.
- `feat(mcp)`: explain impact risk for AI review (`c674d19`) - impact preview returns riskReasons, impactMetrics, confidenceSummary, reviewFocus.
- `feat(mcp)`: associate code changes with docs (`7c19d41`) - static doc graph, DocScanner, code ↔ docs association for AI sidecar.
- `feat(typescript)`: add Phase A local graph support (`fb3719c`) - TypeScript language adapter, symbols, imports, calls.
- `feat(mcp)`: add persistent analysis cache (`c44b51d`) - two-layer cache (memory LRU + persistent disk), fingerprint stale detection, structured staleReasons.

### Changed

- (No breaking changes in this release cycle.)

### Documentation

- (Documentation updates tracked per feature above.)

### Known Limitations

- **TypeScript**: no path alias resolution, no monorepo/workspace support, no TSX framework hints.
- **ArkTS**: struct keyword parsed as ERROR by tree-sitter-typescript, no @Builder/@Extend, no full ArkUI declarative syntax tree.
- **Persistent cache**: no per-symbol incremental recompute.
- **Call edges** are heuristic with confidence/reason, not compiler-verified.
- **No project script execution**.
- **Not a compiler, IDE, language server, or hosted service**.

## [0.1.0] - 2026-05-11

### Added

- Public `codelattice` release binary, while retaining `gitnexus-rust-core-cli` as a compatibility binary.
- Portable release tarball packaging with `manifest.json`, stable MCP wrapper, checksums, docs, and Rust/Cangjie smoke fixtures.
- Release smoke validation for packaged binaries, wrapper self-test, MCP `tools/list`, and portable Rust/Cangjie fixture analysis.
- Fresh clone smoke workflow for external-user setup validation without writing AI client configuration files.
- Portable MCP install/promote scripts with configurable install directories.

### Changed

- README and getting-started docs now present CodeLattice as a standalone local code intelligence engine for Rust and Cangjie projects.
- MCP setup docs and generated config snippets now prefer stable promoted runtime paths over developer checkout wrappers.

### Fixed

- Cangjie `project_overview` compact output now reports nonzero top-level symbol, source file, and edge counts for populated projects.
- Install and promote scripts no longer assume the original author's machine path.

### Compatibility

- The Cargo package and compatibility binary name `gitnexus-rust-core-cli` remain available for existing scripts.
- The public command surface should prefer `codelattice` for new usage.
