# Unified Quality Metrics Pack Preflight

Date: 2026-05-15

## Goal

Add cross-language unified quality metrics (`qualityMetrics`) to MCP tool outputs and real-project corpus baseline comparison.

## New Fields

### qualityMetrics object (added to project_overview, project_insights, review_plan, production_assist)

```json
{
  "graphCompleteness": {
    "nodeCount": 0,
    "edgeCount": 0,
    "symbolCount": 0,
    "sourceFileCount": 0,
    "danglingEdgeCount": 0
  },
  "edgeConfidence": {
    "totalConfidenceEdgeCount": 0,
    "highConfidenceEdgeCount": 0,
    "mediumConfidenceEdgeCount": 0,
    "lowConfidenceEdgeCount": 0,
    "unknownConfidenceEdgeCount": 0,
    "lowConfidenceEdgeRate": 0.0,
    "unknownConfidenceEdgeRate": 0.0
  },
  "callQuality": {
    "callEdgeCount": 0,
    "highConfidenceCallEdgeCount": 0,
    "mediumConfidenceCallEdgeCount": 0,
    "lowConfidenceCallEdgeCount": 0,
    "unknownConfidenceCallEdgeCount": 0,
    "lowConfidenceCallRate": 0.0
  },
  "dependencyQuality": {
    "importEdgeCount": 0,
    "includeEdgeCount": 0,
    "unresolvedImportOrIncludeCount": 0
  },
  "diagnostics": {
    "diagnosticCount": 0,
    "unresolvedDiagnosticCount": 0,
    "parseDiagnosticCount": 0
  },
  "generatedFrom": {
    "graphBased": true,
    "compilerVerified": false,
    "heuristic": true
  }
}
```

## Compatibility

- Only adds new `qualityMetrics` field to existing outputs
- Never deletes or renames existing fields
- Rates return 0.0 when denominator is 0 (no NaN/null)
- Missing edge kinds return 0 counts, not omitted fields

## Confidence Tiers

- >= 0.80 → high
- >= 0.60 → medium
- < 0.60 → low
- missing/null → unknown

## Edge Kind Classification

- CALLS → call edges
- IMPORTS / IMPORT / USE / REFERENCES (import-like) → importEdgeCount
- INCLUDES / INCLUDE → includeEdgeCount
- Other → counted but not classified as import/include

## Write Set

- `crates/cli/src/mcp_server.rs` — add `compute_quality_metrics()`, wire into 4 handlers
- `crates/cli/tests/mcp_server.rs` — add qualityMetrics tests
- `scripts/real-project-corpus-smoke.py` — add qualityMetrics to results + compare
- `scripts/real-project-corpus-smoke-test.py` — add qualityMetrics budget tests
- `docs/real-project-corpus-baseline.json` — add qualityMetrics per target
- `docs/real-project-corpus.md` — document quality budget
- `README.md` — add quality metrics section
- `docs/architecture/mcp-v0-contract.md` — document qualityMetrics field
- `docs/architecture/unified-output-contract.md` — document cross-language fields
- `CHANGELOG.md` — Unreleased section
- `docs/plans/README.md` — add pack index

## Forbidden Set

- Do NOT modify GitNexus-RC / Tool / WebUI
- Do NOT modify real project source code
- Do NOT add new dependencies
- Do NOT change existing MCP field semantics
- Do NOT change language analyzer output to make metrics look better
- Do NOT make low-confidence a hard failure gate

## Stop-line

- If cargo test fails → fix before proceeding
- If MCP tool outputs break existing consumers → stop and reassess

## Verification Plan

1. cargo fmt --check
2. cargo test --test mcp_server
3. python3 scripts/real-project-corpus-smoke-test.py
4. Tool detect-changes
5. Tool index refresh
