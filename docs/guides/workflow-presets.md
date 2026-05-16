# CodeLattice Workflow Presets

`codelattice_workflow_presets` returns recommended MCP tool sequences for
common engineering scenarios. It is a planning tool only: it does not run
analysis, read the target project, or claim that checks have already passed.

## Generated From

Every preset should be interpreted as:

```json
{
  "presetOnly": true,
  "analysisExecuted": false,
  "runtimeVerified": false
}
```

## Common Stop-Lines

- Do not delete code based only on `codelattice_dead_code_candidates`.
- Do not treat framework entry hints as runtime proof.
- Do not treat external API surface as proof of real external usage.
- Run project tests/builds outside CodeLattice when the project requires them.
- Use `confidence`, `reason`, `cautions`, and `generatedFrom` fields when
  deciding whether to trust a result.

## Scenarios

### `onboarding`

Use when entering an unfamiliar repository.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_project_insights` | `readFirst`, `reviewFirst`, `riskMap`, `lowConfidenceZones` |
| 2 | `codelattice_reachability_map` | `entryPoints`, `reachable`, `unreachableCandidates` |
| 3 | `codelattice_external_api_surface` | `externalSurfaceSymbols`, `cautionLevel`, `recommendedVerification` |
| 4 | `codelattice_framework_entry_hints` | `frameworkEntryHints`, `hintKind`, `cautions` |
| 5 | `codelattice_review_plan(mode=onboarding)` | `readPlan`, `riskReviewPlan`, `recommendedMcpCalls` |

### `before_edit`

Use before changing a symbol, file, public API, or framework entry.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_symbol_context` or `codelattice_symbol_search` | candidate identity, file, line, snippets |
| 2 | `codelattice_impact_preview` | `risk`, `riskReasons`, `impactMetrics`, `reviewFocus` |
| 3 | `codelattice_breaking_change_review` | `compatibilityRisk`, `changedExternalApi`, `changedFrameworkEntries` |
| 4 | `codelattice_review_plan(mode=before_edit)` | checklist and stop-lines |

### `after_edit`

Use after local edits and before commit.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_changed_symbols` | touched symbols and unknown hunks |
| 2 | `codelattice_impact_preview` | changed-symbol risk and affected files |
| 3 | `codelattice_breaking_change_review` | public API and framework compatibility risk |
| 4 | `codelattice_consistency_review` | stale docs, related tests, missing tests |
| 5 | `codelattice_config_examples_review` | stale examples, config references, scripts |
| 6 | `codelattice_review_plan(mode=after_edit)` | final AI review checklist |

### `delete_code`

Use before deleting functions, classes, files, packages, or old modules.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_dead_code_candidates` | candidate score, confidence, cautions |
| 2 | `codelattice_reachability_map` | entry reachability and unreachable candidates |
| 3 | `codelattice_external_api_surface` | external consumer cautions |
| 4 | `codelattice_framework_entry_hints` | route/callback/component cautions |
| 5 | `codelattice_impact_preview` | upstream callers and affected paths |
| 6 | `codelattice_review_plan(mode=before_edit)` | deletion checklist |

Stop if a candidate has public API, framework entry, dynamic dispatch, or
ambiguous symbol cautions. That does not mean the code is live, but it blocks
automatic deletion.

### `release_check`

Use before packaging or publishing.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_quality` | failed quality gates |
| 2 | `codelattice_project_overview` | `qualityMetrics` and graph completeness |
| 3 | `codelattice_breaking_change_review` | compatibility risk and release-note hints |
| 4 | `codelattice_consistency_review` | stale docs/tests and missing tests |
| 5 | `codelattice_config_examples_review` | stale config/scripts/examples |
| 6 | `codelattice_review_plan(mode=release_check)` | release readiness checklist |

### `legacy_cleanup`

Use when researching a large or tangled legacy codebase.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_project_insights` | hotspots, read-first files, low-confidence zones |
| 2 | `codelattice_dead_code_candidates` | suspected dead symbols/files |
| 3 | `codelattice_reachability_map` | entry reachability gaps |
| 4 | `codelattice_external_api_surface` | externally visible APIs |
| 5 | `codelattice_framework_entry_hints` | hidden framework/callback entries |
| 6 | `codelattice_config_examples_review` | stale examples and config drift |

### `public_api_change`

Use before changing exported symbols, public headers, package entry files, or
documented APIs.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_external_api_surface` | public surface score and cautions |
| 2 | `codelattice_breaking_change_review` | compatibility risk |
| 3 | `codelattice_impact_preview` | graph impact and callers |
| 4 | `codelattice_consistency_review` | docs/tests that mention the API |
| 5 | `codelattice_review_plan(mode=before_edit)` | compatibility checklist |

### `framework_route_change`

Use before changing route handlers, CLI handlers, callbacks, components, or
lifecycle hooks.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_framework_entry_hints` | route/callback/component hints |
| 2 | `codelattice_reachability_map` | whether framework entries are visible as entries |
| 3 | `codelattice_breaking_change_review` | compatibility and route risk |
| 4 | `codelattice_consistency_review` | route docs and tests |
| 5 | `codelattice_review_plan(mode=before_edit)` | route/callback checklist |

### `docs_tests_sync`

Use when code has changed and docs/tests may need updates.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_changed_symbols` | changed symbols and unknown hunks |
| 2 | `codelattice_consistency_review` | stale docs/tests and missing tests |
| 3 | `codelattice_breaking_change_review` | release-note and compatibility hints |
| 4 | `codelattice_review_plan(mode=after_edit)` | post-edit checklist |

### `config_examples_sync`

Use when package config, examples, scripts, CI, Docker, or documented commands
may be stale.

| Step | Tool | Inspect |
|------|------|---------|
| 1 | `codelattice_config_examples_review` | stale config references and examples |
| 2 | `codelattice_consistency_review` | docs/tests relation to changed symbols |
| 3 | `codelattice_breaking_change_review` | compatibility and release-note hints |
| 4 | `codelattice_review_plan(mode=release_check)` | release checklist |
