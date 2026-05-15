# CodeLattice Changelog

All notable CodeLattice release changes are tracked here.

This project follows the release policy in `docs/release-versioning.md`. The product version comes from Cargo `workspace.package.version`; MCP `serverVersion` is a separate runtime/tool-profile version.

## [Unreleased]

### Added

- **MCP tool `codelattice_project_insights`** (v0.8): Large project insight map for AI agents onboarding onto unfamiliar codebases. Identifies entry point candidates, hotspot files/symbols, risk areas, low-confidence zones, and provides read-first/review-first recommendations. Graph-based heuristic — not compiler/IDE-level proof.
  - File metrics: symbolCount, edgeCount, callInCount, callOutCount, lowConfidenceEdgeCount, diagnosticCount, riskScore with reasons
  - Symbol metrics: fanIn, fanOut, crossFileImpactCount, lowConfidenceEdgeCount, isEntryLike, isPublic, riskScore with reasons
  - Risk scoring: weighted composite (fan-in, fan-out, low-confidence edges, cross-file impact, diagnostics) with test/generated/vendor downweighting
  - Entry point detection: language-specific heuristics for Rust (`main`, lib.rs public API, high fan-out orchestrators), Cangjie, ArkTS (`@Entry`, `build()`), TypeScript
  - Sections: `summary`, `entryPointCandidates`, `hotspotFiles`, `hotspotSymbols`, `riskMap`, `lowConfidenceZones`, `readFirst`, `reviewFirst`, `docsSignals`
  - `compact=true` (default): id/name/kind/file/line/riskScore/reasons only
  - `compact=false`: adds `fileMetrics` breakdown and extra summary fields
  - `limit` parameter controls max items per category
  - `includeDocs`/`includeDiagnostics` toggle doc and diagnostic signals
  - `generatedFrom`: `graphBased=true, compilerVerified=false, previewOnly=true`
- New smoke script: `scripts/project-insights-smoke.sh` (15 checks)
- Updated `scripts/mcp-dogfood.sh` to include `codelattice_project_insights` (23 tool checks)
- 7 new MCP integration tests for project_insights
- Updated README.md: large project insight section, AI-sidecar workflow step 1

- **C Phase A** (unreleased): C language static analysis support via tree-sitter-c.
  - New `gitnexus-c` crate: `extractors/` (symbol, include), `graph.rs`, `project.rs`
  - CLI `analyze --language c`, `quality --language c`, `summary --language c` commands
  - MCP `language=c` enum added to all 21 tool schemas; `check_language_feature` updated
  - Bridge format: `convert_c_graph` independent implementation
  - Auto-detect: walk directory tree, exclude C++ files (`.cpp/.cc/.cxx/.hpp/.hh/.hxx`)
  - Phase A limitations: no macro expansion, no function pointer resolution, no C++ support
  - New `scripts/c-real-project-smoke.sh`: synthetic + real project C smoke tests
  - 9 new feature-gated MCP integration tests for C language
  - `serverInfo.cSupport` profile flag in MCP initialize response

- **MCP tool `codelattice_review_plan`** (v0.9): AI engineering review checklist that synthesizes project insights, impact analysis, changed symbols, and doc associations into actionable plans. Graph-based heuristic — not compiler/IDE/test-system proof.
  - 4 modes: `onboarding` (start a new project), `before_edit` (pre-change impact preview), `after_edit` (post-change impact + test/doc hints), `release_check` (pre-release quality gate)
  - Each plan item: priority (P0/P1/P2), action, target, file, line, reason, source, recommendedTool, doneCriteria
  - `onboarding`: readPlan (entry points, hotspot files, docs), riskReviewPlan (high-risk symbols), recommendedMcpCalls
  - `before_edit`: impactPreview (callers, file ripple), backwardCompatNotes, questionPrompt (if no symbol), recommendedMcpCalls
  - `after_edit`: impactSummary, testHints, docUpdateHints (via DocScanner), recommendedMcpCalls
  - `release_check`: qualityGates, diagnosticSummary, lowConfidenceEdges, testHints, releaseReadiness, recommendedMcpCalls
  - Parameters: root, language, mode, symbol, changedSymbols, compact, limit, includeDocs, includeTests
  - `generatedFrom`: `graphBased=true, compilerVerified=false, previewOnly=true`
- Updated `scripts/mcp-dogfood.sh` to include `codelattice_review_plan` (24 tool checks)
- Updated `scripts/codelattice-mcp.sh` tool threshold to >=24
- Updated `scripts/install-mcp.sh` tool count to 24
- 8 new MCP integration tests for review_plan (onboarding basic/entry_points/docs_signal, before_edit with/without symbol, after_edit, release_check, invalid mode)
- Updated README.md: review_plan in tools table, AI workflow expanded to 8 steps integrating review_plan

### Changed

- (No unreleased changes yet.)

### Fixed

- Local promotion, install, and release packaging scripts now build and validate the full optional language set: Cangjie, ArkTS, TypeScript, C, C++, and Python. This prevents promoted `CodeLattice-Tool` or tarball artifacts from silently losing the newer C/C++/Python adapters.

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
