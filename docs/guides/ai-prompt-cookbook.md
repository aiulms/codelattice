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

Do not call the beta GA-ready just because static checks pass.
```

## 10. Investigate A Runtime Bug With Evidence

```text
Use CodeLattice to move this bug from guessing toward an evidence-backed root
cause hypothesis.

Root: <repo-root>
Language: <language or auto>
Issue: <short bug description>
Observed error/log/screenshot summary: <optional>
Reproduction steps: <optional>
Available AI capabilities: <read_code/read_git_diff/run_commands/read_logs/browser/local_http/edit_code/runtime_probe/trace_files>

Call codelattice_workflow with mode=root_cause first. It should route through
codelattice_change_review mode=root_cause in the default six-tool MCP surface.
If you already
have permission to read logs, run commands, inspect local HTTP/debug endpoints,
operate a browser, or edit temporary probes, use the returned nextBestAction
instead of asking me to manually assemble evidence.

Return:
- the top rootCauseHypotheses with confidence;
- evidence already supporting each hypothesis;
- missingEvidence and why it matters;
- the smallest next evidence action;
- likelyFixArea;
- what is probably a downstream symptom, not root cause;
- nextVerification after a fix.

Do not claim runtime proof unless runtime evidence was actually provided. Do
not install probes or change source through CodeLattice; use the AI client's
normal edit permissions and keep probes minimal/reversible.
```

## 11. Legacy Cleanup Plan

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

## 12. Compact Prompt For Daily AI Coding

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

## 13. Prompt For Another Agent

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

## 14. Recover From mcp_server_busy

```text
A CodeLattice MCP call returned codelattice.mcpBusy.v2 (error
mcp_server_busy). Do not retry immediately.

Inspect the response fields: retryAfterSeconds, recommendedNextCalls, and
aiGuidance. Then:
1. Wait for the current in-flight call to finish before the next call.
2. Do not fire multiple CodeLattice tool calls concurrently in this session.
3. If the busy error persists after the in-flight call completes, recommend
   restarting the MCP session (disable/re-enable the codelattice server in the
   client, or restart the client).
4. For large repositories, switch to job mode instead of long synchronous
   calls: codelattice_project(mode=job) or codelattice_workspace(mode=job),
   then poll mode=job_status, then read pages via mode=job_detail.

Report: which call was busy, how long to wait, and the follow-up plan.
```

## 15. Recover From tool_not_in_ai_toolset

```text
A CodeLattice MCP call returned tool_not_in_ai_toolset (or
tool_not_in_core_toolset). The tool is hidden in the current toolset.

Do not enable CODELATTICE_MCP_TOOLSET=full unless the user explicitly asks for
debug mode. Instead, map the hidden tool to its facade equivalent:

- codelattice_project_insights / project_overview / quality
  → codelattice_project(mode=insights | overview | quality)
- codelattice_symbol_search / symbol_context
  → codelattice_symbol(mode=search | context)
- codelattice_impact_preview / dead_code_candidates / reachability_map /
  external_api_surface / framework_entry_hints / breaking_change_review /
  changed_symbols / consistency_review / config_examples_review
  → codelattice_change_review(mode=impact | dead_code | reachability |
     external_api | framework_entries | breaking_change | changed_symbols |
     consistency | config_examples)
- codelattice_review_plan(mode=...)
  → codelattice_workflow(mode=<same scenario>)

If the error message names a recommended entry tool, use that. Report which
hidden tool was requested and which facade call replaced it.
```

## 16. Recover From execution.status=needs_input

```text
A codelattice_workflow call with execute=true returned
execution.status=needs_input. Analysis stopped on purpose.

Do not guess the missing input. Read execution.missingInputs and
execution.nextActions in order:
1. For a missing symbol, run the suggested codelattice_symbol(mode=search)
   discovery action, pick the best candidate, then re-run the workflow with
   that symbol.
2. For a missing root/target, run the suggested discovery action
   (codelattice_workspace(mode=graph) for cross-project targets), then re-run
   with the resolved target.
3. If the search returns multiple same-name candidates, disambiguate by
   file/kind/line before proceeding; do not assume the first hit.

Report: what was missing, which discovery action you ran, and the resolved
value you used for the retry.
```

## 17. Handle Symbol Disambiguation

```text
A symbol search/context/impact call returned multiple same-name candidates
(e.g. several <symbol> across different files). Do not pick one blindly.

1. Compare candidates by file path, kind, and line.
2. Use codelattice_symbol(mode=context, name=<symbol>) with file_path or kind
   hints to narrow down, or codelattice_change_review(mode=impact,
   symbol=<symbol>, ...) against the file that matches the current task.
3. If the current edit/review target is known from the user's context, prefer
   the candidate in that file.
4. When in doubt, ask the user which one they mean before any impact review or
   edit.

Report: the candidates you found, how you disambiguated, and whether you
needed to ask the user.
```

## 18. Handle Cache Stale / Job Not Ready

```text
CodeLattice returned a stale-cache signal (staleReasons / scheduler
reuse=fresh) or codelattice.jobNotReady.v1.

For stale cache: read staleReasons to see which files changed and which
phases are affected. The next analysis will re-run; treat the previous cached
result as outdated for the changed files. If you only need symbol-level data,
codelattice_symbol(mode=search) may still reuse the cache delta where safe.

For jobNotReady: the job is still queued/running. Poll
codelattice_project(mode=job_status, jobId=<id>) with a short delay, and when
status=succeeded read codelattice_project(mode=job_detail, jobId=<id>, page=0,
pageSize=50). Do not submit a duplicate job with the same root.

Report: the stale reason or job progress, and what you did next.
```
