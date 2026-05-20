# Analysis Scheduler / Incremental Core Design

Date: 2026-05-20
Status: Approved by user direction

## Goal

Introduce a small parser-agnostic scheduler core for CodeLattice so analysis can be described as deterministic phases with cache and staleness metadata.

## Scope

The first slice builds a reusable Rust crate and wires its metadata into existing MCP cache flows. It does not parallelize real adapter execution, add language semantics, execute project code, or replace tree-sitter.

## Architecture

The scheduler core owns request normalization, cheap filesystem fingerprints, deterministic phase planning, and cache decision labels. The CLI/MCP layer keeps ownership of subprocess execution and cache storage.

```text
MCP/CLI request
  -> AnalysisRequest
  -> AnalysisFingerprint
  -> AnalysisJobPlan
  -> AnalysisSchedule
  -> existing analyze/cache execution
```

## Components

- `gitnexus-analysis-scheduler`: internal crate with serializable request, fingerprint, phase, and schedule types.
- MCP cache integration: records schedule metadata in cache status and prewarm responses.
- Tests: crate-level unit tests for deterministic plans and MCP tests for surfaced metadata.

## Success Criteria

- Scheduler plans are stable for the same request.
- Different source mtimes/sizes produce different fingerprints.
- Cache status exposes scheduler metadata without triggering analysis.
- Prewarm/analyze cache metadata identifies whether work was reused or freshly run.
- No target project scripts are executed.

