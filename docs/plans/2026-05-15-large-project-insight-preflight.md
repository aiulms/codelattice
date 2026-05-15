# Plan: Large Project Insight Pack

**Date**: 2026-05-15
**Status**: In Progress
**Version Target**: 0.13.0-beta.2 → unreleased

## Goal

Enhance CodeLattice's ability to help AI agents and humans understand large, unfamiliar codebases ("屎山代码"). Add a new MCP tool `codelattice_project_insights` that provides:

1. File-level metrics (symbols, edges, calls, diagnostics, risk scores)
2. Symbol-level metrics (fan-in, fan-out, cross-file impact, entry-like detection)
3. Entry point candidates (language-specific heuristics)
4. Hotspot files and symbols (ranked by composite risk score)
5. Risk map (top risky items with suggested review actions)
6. Low-confidence zones (files/symbols with many uncertain edges)
7. Read-first and review-first recommendations (with reasons)
8. Docs signals (symbol ↔ doc associations)

## Key Design Decisions

- **Risk scoring is weighted composite, not ML**: Weights are transparent and documented. Each reason is a separate field. No black box.
- **Entry point detection is language-specific**: Rust (`main`, lib.rs, high fan-out), Cangjie (`main`, package root), ArkTS (`@Entry`, `build()`), TypeScript (`index.ts`, `main.ts`, TSX).
- **Test/generated/vendor files are downweighted**: Score multiplied by 0.3-0.5 for generated/vendor/test files.
- **Compact mode by default**: Only id/name/kind/file/line/riskScore/reasons per item. Full mode adds fileMetrics breakdown.
- **Graph-based heuristic disclaimer**: `generatedFrom.graphBased=true, compilerVerified=false` on every response.

## Files Changed

- `crates/cli/src/mcp_server.rs`: +600 lines (handle_project_insights + detect_entry_like)
- `crates/cli/tests/mcp_server.rs`: +7 tests
- `scripts/mcp-dogfood.sh`: +1 tool check
- `scripts/project-insights-smoke.sh`: New script
- `README.md`: Updated sections
- `CHANGELOG.md`: Updated
- `docs/architecture/mcp-v0-contract.md`: Updated (via CHANGELOG)
- `docs/plans/`: This file + closure

## Stages

1. ✅ Stage 0: Truth Gate
2. ✅ Stage 1: Insight Metrics Model
3. ✅ Stage 2: New MCP Tool
4. ✅ Stage 3: Entry Point Candidates
5. ✅ Stage 4: Hotspot Files/Symbols
6. ✅ Stage 5: Risk Map / Low Confidence Zones
7. ✅ Stage 6: Read First / Review First
8. ✅ Stage 7: Integrate Existing Tools (reuses GraphView, DocScanner, diagnostics_for)
9. ✅ Stage 8: Tests (7 new MCP tests, 96/96 total)
10. ✅ Stage 9: Scripts/Smoke (15/15 project-insights-smoke, 23/23 dogfood)
11. ✅ Stage 10: Documentation
12. Stage 11: Full Verification
13. Stage 12: Tool Index Refresh
14. Stage 13: Commit/Push
