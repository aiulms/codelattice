# CodeLattice MCP AI Usage Guide

## Recommended MCP Configuration

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh"
    }
  }
}
```

During development, you can point directly to the debug binary:

```json
{
  "mcpServers": {
    "codelattice": {
      "command": "/Users/jiangxuanyang/Desktop/codelattice/target/debug/codelattice",
      "args": ["mcp"]
    }
  }
}
```

## Toolset

### Default AI Toolset (6 tools)

Do NOT set `CODELATTICE_MCP_TOOLSET=full` in daily usage. The default 6 facade tools are the recommended AI toolset:

| Tool | Modes | Purpose |
|------|-------|---------|
| `codelattice_workflow` | ask / onboarding / before_edit / diagnose_issue / explore | Natural language routing and multi-step orchestration |
| `codelattice_project` | quick / standard / deep / insights / job | Project-level analysis at varying depth |
| `codelattice_symbol` | search / context / call_chains / job | Symbol lookup, context, and call chain tracing |
| `codelattice_change_review` | before_edit / after_edit / impact / breaking_change / job | Pre/post edit risk assessment |
| `codelattice_workspace` | overview / graph / job | Monorepo/multi-project workspace analysis |
| `codelattice_cache` | status / clear / explain | Cache management and explanation |

### Full Toolset (49 tools)

The full toolset is for debugging and development only. It exposes all internal tools including low-level analysis engines. Setting `CODELATTICE_MCP_TOOLSET=full` increases token usage and tool selection complexity for AI assistants.

## Recommended Workflows

### Choosing The Right Facade

If you are unsure, start with `codelattice_workflow(mode=ask)` or `codelattice_workflow(mode=explore)`.

Use the facades by decision stage:

| Stage | Use | Avoid |
|-------|-----|-------|
| Unsure what to ask | `codelattice_workflow` | Guessing between project/symbol/change_review |
| Workspace or monorepo root | `codelattice_workspace` first | Passing workspace root directly to symbol/change_review |
| Project orientation | `codelattice_project(mode=quick)` | Starting with deep/full payloads |
| Known symbol or name | `codelattice_symbol` | Using project mode to hunt through long lists |
| Concrete edit target | `codelattice_change_review(mode=impact)` | Treating workflow before_edit as the final review |

### New Project

```
codelattice_workflow(mode=ask, question="了解这个项目")
```
or
```
codelattice_project(mode=quick)
```

### Large Project / Monorepo

```
codelattice_workspace(mode=overview)
```
Then select a `recommendedProjectRoots` entry and run `codelattice_project(mode=standard)` on it.

### Understanding Execution Flow

```
codelattice_workflow(mode=ask, question="helper 的执行流程是什么")
```
or directly:
```
codelattice_symbol(mode=call_chains, query="helper", direction="both")
```

### Locating A Bug Or Symptom

For natural-language issue triage, use one `ask` call first:

```
codelattice_workflow(mode=ask, question="mission_loop 报错怎么定位", compact=true)
```

Issue-like questions return a compact `triagePlan` when CodeLattice has enough static evidence. The plan includes `likelyAreas`, `readFirst`, `hypotheses`, `impactHints`, and `evidenceGaps`. Treat this as a static investigation lead: CodeLattice did not reproduce the bug, run tests, execute the target project, or prove coverage.

After choosing a likely symbol or file, continue with:

```
codelattice_symbol(mode=call_chains, query="mission_loop")
codelattice_change_review(mode=impact, symbol="mission_loop")
```

### Whatif / Pre-Edit Change Preview

Before making changes, use `whatif` to preview impact without actually modifying code:

```
codelattice_change_review(mode=whatif, change="删除 helper 函数", root="/path/to/project", language="rust")
```

Or via ask:
```
codelattice_workflow(mode=ask, question="如果删除 helper 会影响什么")
```

Whatif returns `codelattice.whatIf.v1` with:
- `targetCandidates` — symbols matching the change target
- `directImpact` — direct callers/callees affected
- `indirectImpact` — transitive dependencies
- `risk` — level (low/medium/high/critical) with reasons
- `safeAlternatives` — suggested safer approaches
- `testsToRun` — recommended test validation steps

All whatif results are static-only. `targetCodeExecuted=false` means CodeLattice did not run or build the target project.

### Compact Payloads

Use `compact=true` by default when asking for orientation, call chains, or issue triage. Compact facade responses intentionally keep `rootDiagnosis` small: they include `sourceOnlySummary` and at most five `sourceOnlyEntryPreview` items, but omit full `sourceOnlyEntries`.

Compact facade responses include `decisionGuidance.compactSemantics`, which lists the fields that were kept and omitted. Treat compact output as safe for routing and first-pass risk decisions; switch to `compact=false`, `deep`, or `job_detail` when you need full evidence lists.

Use `compact=false` only when you explicitly need full source-only directory diagnostics or full result payloads.

## Decision Guidance Fields

Most facade responses include `decisionGuidance`:

```json
{
  "toolRole": "single-project structure and risk map",
  "rootKind": "single_project",
  "recommendedNextTool": "codelattice_project mode=standard",
  "modeSemantics": {
    "does": "Fast static orientation...",
    "doesNot": "Does not run tests..."
  }
}
```

Use this object to avoid guessing tool boundaries. `workflow` is the router, `project` is orientation/risk mapping, `symbol` is symbol lookup and call relationships, `change_review` is concrete edit review, and `workspace` is monorepo boundary analysis.

## Source-Only Entries

`sourceOnlyEntries` are not manifest-backed projects. They are directories with analyzable source files but no supported project manifest. They now carry:

- `manifestBacked=false`
- `recommendedAsProjectRoot=false`
- `drillDownCandidate=true` only when they are useful as focused sub-area roots
- `selectionGuidance` explaining whether to prefer a parent manifest-backed project

When choosing a project root, prefer `recommendedProjectRoots` or `primaryProjectRoots`. Use source-only entries only for focused drill-down after orientation.

## Risk Ranking

Risk lists can contain many `high` items in large projects. Prefer the ranking fields over the label alone:

- `priorityRank`: lower is more urgent within that result set.
- `relativePriority`: `top`, `peer-high`, `elevated`, or `baseline`.
- `riskCalibration.rawRiskLevel`: the absolute static score bucket.
- `riskCalibration.rankAdjustedRiskLevel`: the calibrated bucket after comparing peers in this result set.
- `riskCalibration.calibratedRiskLevel`: same as rank-adjusted risk, kept for agents that look for a direct calibrated risk field.
- `riskCalibration.calibratedPriorityBand`: `primary`, `secondary`, `watch`, or `baseline`.
- `riskCalibration.percentileBand`: where the item sits in the returned list.
- `riskCalibration.tieBreaker`: why this item appears before another equal-looking item.
- `riskDrivers`: why the item ranked highly, such as `fan_in`, `fan_out`, `cross_file_impact`, `low_confidence`, or `diagnostics`.
- `riskScoreInterpretation`: a short static-only explanation.

Static risk is not runtime proof. Use it to decide read/review order, then confirm with source reads and targeted tests.

### Before Editing Code

```
codelattice_workflow(mode=ask, question="如果删除 helper 会影响什么")
```
or:
```
codelattice_change_review(mode=before_edit, symbol="helper")
```

### Long-Running Analysis on Large Projects

Use job mode for large projects:
```
codelattice_project(mode=job, root="/path/to/project")
```
Then check status:
```
codelattice_project(mode=job_status, jobId="...")
```
And get results:
```
codelattice_project(mode=job_detail, jobId="...", page=0, pageSize=50)
```

## Concurrency

- Do NOT send multiple CodeLattice MCP tool calls in parallel. The server processes requests sequentially.
- If a previous call is still running, wait for it to complete.
- For long-running tasks, use `job` mode which returns immediately with a job ID.

## Static Analysis Semantics

All CodeLattice output includes `analysisSemantics`:

```json
{
  "staticAnalysis": true,
  "targetCodeExecuted": false,
  "runtimeVerified": false,
  "scriptsExecuted": false
}
```

This means:
- `targetCodeExecuted=false` does NOT mean the analysis failed. It means we did not run the target project's code.
- `runtimeVerified=false` means no runtime testing was performed.
- Do NOT treat static analysis results as test coverage or runtime proof.
- Call chains, impact analysis, and framework hints are heuristics based on static graph traversal.

## Ask Mode Intents

`codelattice_workflow(mode=ask)` routes natural language questions to specialized workflows:

| Keywords | Intent | Action |
|----------|--------|--------|
| 流程/调用链/执行路径/call flow/trace | explain_flow | Runs call_chains, returns readOrder |
| 在哪/找/搜索/where/find | find_symbol | Returns symbol search results |
| 了解项目/项目结构/架构/overview | inspect_project | Returns project quick overview |
| 修改/删除/重命名/影响/风险/change/delete | before_edit | Returns pre-edit guidance |
| 报错/异常/定位/bug/error/crash | locate_issue | Returns triage plan |
| (other) | general | Returns guidance on how to use tools |

## After Syncing Installed Version

After running `scripts/codelattice-installed-acceptance.sh --sync`, you MUST restart your MCP session (Claude/OpenCode/TRAE) for the new binary to take effect.

Before release or handoff, verify the installed binary is the same commit as the source tree:

```bash
scripts/codelattice-installed-acceptance.sh --require-fresh-installed
```

Without `--require-fresh-installed`, the script reports a stale installed binary as a warning so development dry-runs can still pass while the installed version intentionally lags behind.
