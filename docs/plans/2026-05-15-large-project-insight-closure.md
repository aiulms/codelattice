# Closure: Large Project Insight Pack

**Date**: 2026-05-15
**Status**: Complete
**Commit**: TBD

## Summary

Added `codelattice_project_insights` MCP tool (v0.8) — graph-based heuristic insight map for AI agents onboarding onto unfamiliar large codebases.

## Deliverables

### New MCP Tool: `codelattice_project_insights`
- **Input**: root, language, compact, limit, includeDocs, includeDiagnostics
- **Output**: summary, entryPointCandidates, hotspotFiles, hotspotSymbols, riskMap, lowConfidenceZones, readFirst, reviewFirst, docsSignals, generatedFrom

### Metrics Model
- **File metrics**: symbolCount, edgeCount, callInCount, callOutCount, lowConfidenceEdgeCount, diagnosticCount, riskScore with reasons
- **Symbol metrics**: fanIn, fanOut, crossFileImpactCount, lowConfidenceEdgeCount, isEntryLike, isPublic, riskScore with reasons
- **Risk scoring**: weighted composite with transparent reasons. Test/generated/vendor files downweighted.

### Entry Point Detection (Language-Specific)
- **Rust**: `main`, lib.rs public API, high fan-out orchestrators, main.rs functions
- **Cangjie**: `main`, package root classes, high fan-out functions
- **ArkTS**: `build()`, `Index.ets`, `MainAbility/`, `aboutToAppear()`, high fan-out functions
- **TypeScript**: `main`, `index.ts`, `main.ts`, TSX function components, high fan-out functions

### Risk Map
- Top risky files and symbols with riskScore, reasons, suggestedReviewAction
- Suggested actions: "avoid broad refactor", "review before significant changes", "inspect manually", "run tests before modifying"

### Low Confidence Zones
- fileZones: files with >2 low-confidence edges, example edges, recommended actions
- symbolZones: symbols with >1 low-confidence edges, reasons, recommended actions

### Read First / Review First
- Read first: entry-like symbols + high information density files (not necessarily risky)
- Review first: high fan-in symbols + files with diagnostics + docs-mentioned items

### Docs Signals
- Symbols mentioned in docs → review if changed
- Uses existing DocScanner infrastructure

## Files Changed
1. `crates/cli/src/mcp_server.rs`: +610 lines (handle_project_insights + detect_entry_like + toolCount=23)
2. `crates/cli/tests/mcp_server.rs`: +7 tests (96/96 total)
3. `scripts/mcp-dogfood.sh`: +1 tool check (23/23)
4. `scripts/project-insights-smoke.sh`: New (15/15 checks)
5. `scripts/codelattice-mcp.sh`: toolCount >= 23
6. `scripts/install-mcp.sh`: 23 tools in description
7. `README.md`: Updated sections (large project, tools table, AI workflow)
8. `CHANGELOG.md`: [Unreleased] section with full details
9. `docs/plans/2026-05-15-large-project-insight-preflight.md`: New
10. `docs/plans/2026-05-15-large-project-insight-closure.md`: This file

## Verification Results
- **cargo fmt --check**: PASS
- **git diff --check**: PASS
- **cargo test --test mcp_server**: 96/96 PASS
- **mcp-dogfood.sh**: 23/23 PASS
- **project-insights-smoke.sh**: 15/15 PASS
- **codelattice-mcp.sh --self-test**: PASS (23 tools)
- **mcp-cache-smoke.sh**: 6/6 PASS

## Pre-existing Issues
- `mcp_doc_scanner_excludes_hidden_dirs`: threshold bumped from 200→300 (repo doc count grew to 200)
- 15 compiler warnings (pre-existing dead code in ScannedDoc/DocRef)

## Constraints Met
- ✅ Only modified CodeLattice repo
- ✅ Did not modify GitNexus-RC runtime/schema/WebUI
- ✅ Did not modify GitNexus-RC-Tool
- ✅ Did not modify real projects (CoolMallArkTS, harmony-utils, etc.)
- ✅ No LLM/embedding dependency added
- ✅ No WebUI
- ✅ No project script execution
- ✅ No npm/tsc/vite/next build
- ✅ No destructive git operations
- ✅ No package/bin rename
- ✅ Existing MCP fields preserved (additive only)
- ✅ Explicit disclaimer: graph-based heuristic, not compiler/IDE proof
