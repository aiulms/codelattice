# Graph Diagnostics Pack — Preflight

**Date:** 2026-05-16
**Status:** In Progress
**HEAD:** 02eb59c

## Goal

Package existing static graph capabilities into 5 user-scenario-oriented diagnostic MCP tools, turning CodeLattice from a "code graph analyzer" into a "local code diagnostics engine."

## What we're doing

5 new MCP tools:
1. `codelattice_impact_analysis` — "What breaks if I change X?"
2. `codelattice_risk_hotspots` — "Where are the most dangerous areas?"
3. `codelattice_architecture_drift` — "Has the architecture rotted?"
4. `codelattice_ai_context_pack` — "What context should I feed an AI before editing?"
5. `codelattice_review_gate` — "Does this diff touch dangerous areas?"

## What we're NOT doing

- No new language support or language semantics
- No type inference, trait solving, macro expansion
- No code execution, build, test running
- No WebUI
- No new Cargo dependencies
- No CLI commands (beta: MCP-first, document in README)
- No GitNexus-RC/Tool/CodeLattice-Tool modifications
- No proof/guarantee/deletion-safe language

## Existing capability reuse

| New Tool | Existing to Reuse |
|---|---|
| impact_analysis | `handle_impact_preview` (L4242), `compute_impact_risk_details` (L3879), GraphView.incoming/outgoing |
| risk_hotspots | `handle_project_insights` (L5982) FileMetrics, `compute_quality_metrics` (L2493), `detect_entry_like` (L6827) |
| architecture_drift | GraphView.incoming/outgoing for cycle detection, file-level edge analysis |
| ai_context_pack | `detect_entry_points` (L7997), GraphView traversal, doc scanner |
| review_gate | `handle_changed_symbols` (L5512), `handle_production_assist` (L5556), dead_code_candidates scoring |

## Output contract

All tools follow the dead-code-candidates output pattern:
- `generatedFrom: { staticAnalysisOnly: true, heuristic: true, compilerVerified: false }`
- All conclusions have `reasons` and `cautions`
- All risk/confidence has explainable scores
- No "guaranteed safe" / "safe to delete" / "proof" language

## Write set

- `crates/cli/src/mcp_server.rs` — 5 new tool schemas, handlers, helpers, dispatch entries
- `crates/cli/tests/mcp_server.rs` — 20+ new tests
- `fixtures/typescript/graph-diagnostics/` — new fixture (~8 files)
- `scripts/codelattice-mcp.sh` — update threshold to >= 30
- `scripts/mcp-dogfood.sh` — add 5 new tool checks, update threshold
- `CHANGELOG.md` — Unreleased section
- `README.md` — Graph Diagnostics section
- `docs/architecture/mcp-v0-contract.md` — 5 new tool sections
- `docs/plans/2026-05-16-graph-diagnostics-pack-preflight.md` — this file

## Forbidden set

- No modifications to language adapter crates
- No modifications to GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool
- No new Cargo dependencies
- No CLI subcommands this round
- No release tags or release page modifications

## Stop-line

- If `cargo test --all-features` fails: stop, report
- If any existing test breaks: stop, fix first
- If mcp_server.rs exceeds maintainable size: consider refactoring (but only if natural)

## Verification plan

1. `cargo fmt --check`
2. `git diff --check`
3. `cargo test --test mcp_server` (default + TS feature)
4. `cargo test --all-features`
5. `scripts/codelattice-mcp.sh --self-test`
6. `scripts/mcp-dogfood.sh`
7. Tool index refresh
8. Commit + push gitcode master
