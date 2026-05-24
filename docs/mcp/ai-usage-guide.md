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

### Compact Payloads

Use `compact=true` by default when asking for orientation, call chains, or issue triage. Compact facade responses intentionally keep `rootDiagnosis` small: they include `sourceOnlySummary` and at most five `sourceOnlyEntryPreview` items, but omit full `sourceOnlyEntries`.

Use `compact=false` only when you explicitly need full source-only directory diagnostics or full result payloads.

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
