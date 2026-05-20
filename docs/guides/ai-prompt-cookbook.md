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

Call codelattice_workflow with mode=onboarding and execute=true first. Inspect
execution/completedActions/evidence. If execution.status=needs_input, follow
missingInputs and nextActions instead of guessing individual low-level tools.

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

Call codelattice_workflow with mode=before_edit, symbol=<symbol>, and
execute=true. Inspect completedActions, failedActions, evidence, riskLevel, and
answerSummary. If the symbol is ambiguous, follow the returned
codelattice_symbol search/context action before impact review.

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

Call codelattice_workflow with mode=after_edit and execute=true. Inspect
completedActions, failedActions, evidence, and answerSummary, then run any
remaining optional nextActions if needed.

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

Call codelattice_workflow with mode=delete_code, symbol=<symbol-or-file>, and
execute=true. This mode should return high caution and safeToProceed=no.
Review completedActions/evidence, then follow any remaining nextActions for
manual verification.

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

Call codelattice_workflow with mode=public_api_change and symbol=<symbol>.
Follow nextActions for external API surface, breaking-change review, impact,
consistency, and review planning.

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

Call codelattice_workflow with mode=framework_route_change and symbol=<symbol>.
Follow nextActions for framework-entry hints, reachability, breaking-change,
consistency, and review planning.

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

Call codelattice_workflow with mode=docs_tests_sync. Follow nextActions for
changed symbols, consistency, breaking-change, and after-edit review planning.

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

Call codelattice_workflow with mode=config_examples_sync. Follow nextActions
for config/examples, consistency, breaking-change, and release-check planning.

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

Call codelattice_workflow with mode=release_check. Follow nextActions for
quality, project overview, breaking-change, consistency, config/examples, and
release-check planning.

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

Call codelattice_workflow with mode=legacy_cleanup. Follow nextActions for
project insights, cleanup candidates, reachability, public API surface,
framework entries, and config/examples risks.

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
1. Run codelattice_workflow mode=before_edit.
2. If missingInputs asks for a symbol, follow the returned codelattice_symbol
   nextAction.
3. Run the returned impact and breaking-change nextActions.

After editing:
1. Run codelattice_workflow mode=after_edit.
2. Follow its returned nextActions for changed symbols, impact,
   breaking-change, consistency, and config/examples review.
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

Start by calling codelattice_workflow for the mode that matches the task. Read
missingInputs first; if anything is missing, run the suggested discovery action
instead of guessing. Then follow nextActions in order. Do not skip cautions or
stop-lines. When reporting, include:
- tools called;
- missingInputs resolved;
- nextActions followed;
- fields inspected;
- risks and cautions;
- recommended manual verification;
- what remains uncertain.

Never claim CodeLattice proves runtime behavior, external usage, test coverage,
or deletion safety.
```
