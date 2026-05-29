# MCP facade warm and digest preflight

Date: 2026-05-29
HEAD before work: 2faef378d9ce8b7d2ae3bf5f88ebee55019c0087

## Scope

Fix the MCP job close-loop for large real Rust projects so clean-cache symbol search can return `analyzing` quickly, job status reports truthful warm wall-clock/progress, and succeeded job summaries expose non-empty facade graph digest data.

## Risk assessment

Native CodeLattice impact preview on `crates/cli` symbol `build_warm_cache_entry_from_result` reported MEDIUM risk. Blast radius is MCP job execution, facade cache warming, compact facade summaries, and Rust CALLS indexing performance. Static analysis only; runtime verification will be the requested MCP tests and read-only `open-nwe` smoke.

## Write set

- `crates/cli/src/mcp_job.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/project-model/src/calls.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-open-nwe-readonly-smoke.sh`
- `docs/plans/2026-05-29-mcp-facade-warm-digest-preflight.md`

## Forbidden set

- `/Users/jiangxuanyang/Desktop/open-nwe/**` writes
- GitNexus-RC runtime, adapter, graph schema, or package changes
- destructive git operations
- `npx gitnexus` governance

## Stop-line

If native precommit or detect-changes reports HIGH/CRITICAL risk, stop and report before commit/push. If open-nwe git status changes during smoke, stop and report. If warm wall-clock cannot be reduced below 20s, job status/summary must expose structured warm timing so `elapsedMs` cannot be misread as total wall-clock.

## Approach

1. Add failing MCP tests for non-empty `facadeDigest`/AI digest and compact symbol top-match summary.
2. Optimize Rust call resolution indexes where the full graph path is spending most of its time.
3. Extend `WarmCacheMeta` and job progress/summary with wall-clock warm stage timings and facade graph digest.
4. Strengthen the read-only open-nwe smoke to assert digest fields, retry search, impact, toolset counts, and unchanged git status.
5. Run the required verification, then commit, push, sync installed CodeLattice-Tool, and verify the installed manifest commit.
