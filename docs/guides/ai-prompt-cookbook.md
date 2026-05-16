# CodeLattice AI Prompt Cookbook

This cookbook gives copyable prompts for AI assistants that can use the
CodeLattice MCP server. The prompts are intentionally explicit about safety:
CodeLattice does static graph analysis, not runtime proof.

Replace placeholders like `<repo-root>`, `<language>`, and `<symbol>` before
using a prompt.

## 1. Onboard An Unfamiliar Project

```text
Use CodeLattice to help me onboard this repository.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=onboarding first. Then follow
the suggested tools in order:
- project_insights
- reachability_map
- external_api_surface
- framework_entry_hints
- review_plan mode=onboarding

Return:
- the first 5 files I should read;
- likely entry points;
- high-risk symbols/files;
- public API surfaces;
- framework/callback entries;
- low-confidence zones;
- what not to assume.

Do not claim runtime proof. Treat all findings as static graph evidence.
```

## 2. Before Editing A Function Or Class

```text
Before I edit `<symbol>`, use CodeLattice to assess risk.

Root: <repo-root>
Language: <language or auto>
Target symbol: <symbol>

Call codelattice_workflow_presets with scenario=before_edit. Then use:
- symbol_context or symbol_search to identify the exact symbol;
- impact_preview for blast radius;
- breaking_change_review for public API/framework compatibility risk;
- review_plan mode=before_edit.

Return:
- direct callers and important callees;
- files likely affected;
- riskReasons and confidenceSummary;
- public API or framework-entry cautions;
- questions I should answer before editing;
- a short checklist for a safe patch.
```

## 3. After Editing Code

```text
I have local code changes. Use CodeLattice for an after-edit review.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=after_edit. Then run:
- changed_symbols using the working tree diff;
- impact_preview for changed symbols;
- breaking_change_review;
- consistency_review;
- config_examples_review;
- review_plan mode=after_edit.

Return:
- changed symbols and unknown hunks;
- compatibility risk;
- likely docs/tests/config updates;
- stale examples or config references;
- recommended tests to run outside CodeLattice;
- a final commit-readiness checklist.

Do not run project tests or builds through CodeLattice.
```

## 4. Review A Possible Dead-Code Deletion

```text
I want to investigate whether `<symbol-or-file>` can be removed.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=delete_code. Then use:
- dead_code_candidates;
- reachability_map;
- external_api_surface;
- framework_entry_hints;
- impact_preview;
- review_plan mode=before_edit.

Return:
- whether it is a dead-code candidate;
- why it might be unused;
- why it might still be unsafe to delete;
- public API cautions;
- framework/callback cautions;
- impact if removed;
- required manual verification.

Do not produce a deletion patch. Do not call it proven dead code.
```

## 5. Review A Public API Change

```text
I plan to change public API `<symbol>`.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=public_api_change. Then use:
- external_api_surface;
- breaking_change_review with changedSymbols=["<symbol>"];
- impact_preview;
- consistency_review;
- review_plan mode=before_edit.

Return:
- whether this symbol looks externally visible;
- compatibilityRisk and riskReasons;
- docs/tests likely affected;
- release-note hints;
- downstream-consumer cautions;
- a safe-edit checklist.

Do not claim actual external usage unless the repository itself proves it.
```

## 6. Review A Framework Route, CLI, Callback, Or Component Change

```text
I plan to change framework or callback entry `<symbol>`.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=framework_route_change. Then use:
- framework_entry_hints;
- reachability_map;
- breaking_change_review with changedSymbols=["<symbol>"];
- consistency_review;
- review_plan mode=before_edit.

Return:
- whether the symbol looks like a route, handler, CLI command, callback,
  component, or lifecycle entry;
- why ordinary call graph analysis may miss callers;
- docs/tests likely affected;
- route/callback verification steps;
- a safe patch checklist.

Do not treat framework hints as runtime proof.
```

## 7. Check Docs And Tests After A Change

```text
Check whether docs and tests are consistent with my current changes.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=docs_tests_sync. Then use:
- changed_symbols;
- consistency_review;
- breaking_change_review;
- review_plan mode=after_edit.

Return:
- staleDocCandidates;
- missingDocUpdateCandidates;
- relatedTests;
- missingTestCandidates;
- staleTestCandidates;
- recommended doc/test updates.

Do not claim test coverage. CodeLattice does not run tests.
```

## 8. Check Config, Scripts, Examples, CI, And Docker

```text
Check whether config, scripts, examples, CI, and Docker files still match the
codebase.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=config_examples_sync. Then use:
- config_examples_review;
- consistency_review;
- breaking_change_review;
- review_plan mode=release_check.

Return:
- staleExamples;
- staleConfigReferences;
- packageScriptRisks;
- entryPointConfigRisks;
- tsconfigPathRisks;
- pythonEntryPointRisks;
- cargoTargetRisks;
- cCppBuildConfigRisks;
- ciConfigRisks;
- recommended fixes and verification commands.

Do not execute scripts, builds, Docker, CI, or package managers.
```

## 9. Run A Release Check

```text
Use CodeLattice for a release-readiness review.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=release_check. Then use:
- quality;
- project_overview;
- breaking_change_review;
- consistency_review;
- config_examples_review;
- review_plan mode=release_check.

Return:
- failed quality gates;
- qualityMetrics summary;
- compatibility risk;
- stale docs/tests/config/examples;
- release-note hints;
- recommended external test/build commands to run manually;
- final go/no-go concerns.

Do not call this GA proof. It is a static release review.
```

## 10. Legacy Cleanup Plan

```text
Help me plan cleanup for a large legacy codebase.

Root: <repo-root>
Language: <language or auto>

Call codelattice_workflow_presets with scenario=legacy_cleanup. Then use:
- project_insights;
- dead_code_candidates;
- reachability_map;
- external_api_surface;
- framework_entry_hints;
- config_examples_review.

Return:
- read-first files;
- high-risk hotspots;
- suspected dead-code clusters;
- public APIs that must not be removed casually;
- framework/callback entries that static call graph may miss;
- stale examples/config risks;
- a phased cleanup plan with safety checks.

Do not delete code automatically. Treat cleanup targets as candidates.
```

## 11. Compact Prompt For Daily AI Coding

```text
Use CodeLattice MCP before and after this edit.

Before editing:
1. Run workflow_presets scenario=before_edit.
2. Identify the target with symbol_context.
3. Run impact_preview and breaking_change_review.

After editing:
1. Run changed_symbols.
2. Run impact_preview, breaking_change_review, consistency_review, and
   config_examples_review.
3. Summarize risks, docs/tests/config updates, and manual test commands.

Follow CodeLattice stop-lines:
- static analysis is not runtime proof;
- dead-code candidates are not deletion proof;
- public API changes need external consumer caution;
- framework entries may hide callers;
- CodeLattice does not execute project code.
```

## 12. Prompt For Another Agent

```text
You are working in a repository with CodeLattice MCP available.

Start by calling codelattice_workflow_presets for the scenario that matches the
task. Follow the returned tool order. Do not skip stopLines. When reporting,
include:
- tools called;
- fields inspected;
- risks and cautions;
- recommended manual verification;
- what remains uncertain.

Never claim CodeLattice proves runtime behavior, external usage, test coverage,
or deletion safety.
```
