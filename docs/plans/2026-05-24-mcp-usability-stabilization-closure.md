# MCP Usability Stabilization Pack Closure

Date: 2026-05-24

## Scope Closed

This stabilization pass focused on making the Analysis Engine 1.3 MCP job runtime usable from real AI clients without adding new exposed tools.

Fixed user-facing problems:

- Installed CodeLattice-Tool was stale and reported runtime language flags that disagreed with `manifest.json`.
- Facade `job_status` and `job_detail` incorrectly required `root`.
- Workspace job responses embedded large project lists in compact mode.
- AI clients could be steered into the full legacy toolset and call low-level tools that bypass the six facade tools.
- Installed MCP smoke coverage skipped the required JSON-RPC initialization path.
- Busy responses did not explain recovery clearly enough for Claude/OpenCode/TRAE users.

## Implementation Summary

- `codelattice_project`, `codelattice_workspace`, `codelattice_symbol`, and `codelattice_change_review` now route `job_status` and `job_detail` before root validation.
- `job_status` requires only `jobId`.
- `job_detail` requires `jobId` and accepts optional `page` / `pageSize`.
- Invalid job ids return structured MCP errors such as `job_not_found` instead of missing-root failures or panics.
- Workspace `mode=job` compact responses now include a small summary, cache summary, `compactResult=true`, `detailPageHint`, and `nextActions`, without embedding complete project, artifact, or diagnostics lists.
- Full details are available through paged `job_detail`.
- `compact=false` still uses bounded response shaping through the job response path.
- Busy errors now say the server is not crashed, explain that another call is still in flight, recommend waiting/retrying, and tell clients to restart the MCP session if busy persists after the original call should have finished.
- The installed smoke script now exercises real MCP JSON-RPC sessions: `initialize`, `notifications/initialized`, `tools/list`, and `tools/call`.

## Installed Tool State

Post-build promotion installed an all-language release build to:

```text
/Users/jiangxuanyang/Desktop/CodeLattice-Tool
```

Observed installed runtime after promotion:

- `serverVersion`: `0.16.0-beta.1`
- Language support: Rust, Cangjie, ArkTS, TypeScript, C, C++, Python, and Shell all report `true`.
- Default AI `tools/list`: exactly 6 facade tools.
- Full `tools/list`: 49 tools.
- `--self-test`: passed.

The installed `sourceCommit` must be refreshed after the final commit because the repository HEAD changes when this closure and the implementation are committed. The final promotion gate is to rerun `promote-to-local-tool.sh --skip-build`, then verify the installed manifest `sourceCommit` equals the final repository HEAD.

## Verification Evidence

Required checks completed during closure:

- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo test --test mcp_server`: passed, 141 tests.
- `cargo test -p gitnexus-analysis-engine`: passed.
- `cargo test`: passed.
- `scripts/codelattice-mcp-concurrency-smoke.sh`: passed with structured busy responses and same-session recovery.
- `scripts/codelattice-installed-mcp-job-smoke.sh`: passed.
- Installed wrapper `--self-test`: passed.
- Native precommit bundle: completed and reported `critical` static-analysis risk due broad MCP/job/script changes. This was reviewed as expected blast radius for this stabilization pack, not an unexpected live-project change.

## open-nwe Read-Only Acceptance

Readonly target:

```text
/Users/jiangxuanyang/Desktop/open-nwe
```

Final readonly smoke result:

- Workspace job compact response: 1499 bytes.
- `job_status(jobId)` worked without `root`.
- `job_detail(jobId, page=0, pageSize=5)` returned paged detail with `totalItems=50`.
- `codelattice_cache(mode=status)` recovered after the workspace job.
- Before/after `git status --short` was unchanged on the final run.
- No target build, test, npm, cargo, or project script was executed.

During an earlier run, the target repository changed concurrently outside this task. The smoke was rerun after the status stabilized and passed with unchanged before/after status.

## Busy And Concurrency

Concurrency smoke verified:

- Overlapping calls return structured `mcp_server_busy` errors or a job handle.
- The MCP connection stays open.
- No `Connection closed` failure was observed.
- No `No such tool available` failure was observed.
- The same session can recover after busy responses.
- Busy did not persist after the tested work completed.

User guidance now documents that persistent busy usually means a client still has a long call hanging in that MCP session, and the fix is to restart the MCP session and prefer job mode for large work.

## AI Client Guidance

Claude, OpenCode, and TRAE should use the default MCP toolset:

```bash
unset CODELATTICE_MCP_TOOLSET
```

Daily AI usage should not set:

```bash
CODELATTICE_MCP_TOOLSET=full
```

The default six facade tools are the supported AI-client surface. `full` is reserved for developer debugging and dogfood.

Recommended large-project calls:

- `codelattice_workspace(mode=job, root=..., language=auto, compact=true)`
- `codelattice_project(mode=job, root=<specific project root>, language=<lang>)`
- `codelattice_* (mode=job_status, jobId=...)`
- `codelattice_* (mode=job_detail, jobId=..., page=0, pageSize=...)`

Do not use low-level full-toolset calls such as `codelattice_project_overview` for monorepos or large workspaces.

After changing MCP environment variables or replacing the installed wrapper, restart Claude/OpenCode/TRAE MCP sessions so the client reloads the tool list and runtime profile.

## Known Limits

- CodeLattice MCP analysis remains static analysis only; it is not compiler, runtime, or coverage proof.
- Job ids are session-local and are not expected to survive MCP server restart.
- Large result detail is intentionally paged through `job_detail`.
- `CODELATTICE_MCP_TOOLSET=full` still exposes development and legacy tools for debugging, so it is intentionally not the daily AI-client mode.
- Method dispatch, external crate handling, macro expansion, cfg evaluation, and type inference remain bounded by the project stop-lines in `AGENTS.md`.
