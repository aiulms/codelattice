# MCP Usability Stabilization Pack Preflight

Date: 2026-05-24

## Goal

Stabilize the Analysis Engine 1.3 MCP job runtime for real Claude, OpenCode, and TRAE usage without adding new exposed MCP tools. The default AI toolset remains the six facade tools.

## Truth Gate Baseline

- Repo HEAD at task start: `9b2a1ee74ece25c655da1c5155276da232572d85`
- Recent commits:
  - `9b2a1ee fix(cache): store workspace project artifacts in persistent cache`
  - `cd07deb feat(engine): add incremental artifact cache with persistence`
  - `f1ae803 fix(engine): resolve relative project paths in workspace scheduler`
  - `8c8806f fix(engine): add recursive file discovery for workspace projects`
  - `f45f7be feat(engine): add workspace multi-project parallel scheduler`
  - `22741ce test(mcp): add installed job runtime acceptance`
  - `539cec2 fix(mcp): verify and sync analysis engine job runtime`
  - `74ddf57 feat(mcp): complete analysis engine job runtime facade`
- Worktree was already dirty at task start in analysis-engine and MCP files. Changes will be reviewed and preserved unless directly incompatible.
- Installed wrapper baseline:
  - `sourceCommit: 539cec2`
  - `serverVersion: 0.16.0-beta.1`
  - runtime language flags from `--version`: Cangjie/ArkTS/TypeScript/JavaScript/C/C++/Python false, Shell true
  - manifest profile claims all language flags true and `toolCount: 51`
  - `--self-test` fails on `cangjieSupport is True`
- Baseline mismatch: installed `sourceCommit` does not match repo HEAD, and manifest language support does not match the installed binary initialize response.

## Root Cause Notes

- `handle_project`, `handle_symbol`, `handle_change_review`, and `handle_workspace` read `root` before dispatching `job_status` or `job_detail`, so rootless status/detail calls return `missing_parameter` instead of looking up `jobId`.
- Workspace job summaries include the full `projects` array. Because `McpJobRegistry::to_response` embeds `summary`, the first `mode=job` response can become large even though `compactResult=true`.
- The installed smoke script is not executable and uses MCP requests without the full initialize + initialized handshake in several places.
- The installed wrapper self-test failure is caused by a stale promoted binary built without all optional language features while manifest metadata was copied from a full-language profile.

## Execution Card

Write set:
- `crates/cli/src/mcp_server.rs`
- `crates/cli/src/mcp_job.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/codelattice-installed-mcp-job-smoke.sh`
- `scripts/codelattice-mcp-concurrency-smoke.sh` if busy verification needs stronger assertions
- `docs/guides/ai-mcp-tool-guide.md`
- `CHANGELOG.md`
- `docs/plans/2026-05-24-mcp-usability-stabilization-closure.md`
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` through `scripts/promote-to-local-tool.sh`

Forbidden set:
- Do not modify open-nwe, cangjie, GitNexus-RC, warp, openfang, or other live project source.
- Do not execute target project build/test/npm/cargo scripts for open-nwe.
- Do not expose new MCP tools.
- Do not make destructive git operations.

Stop-line:
- Stop and warn before commit/push if native CodeLattice detect-changes or impact review reports high or critical risk.
- Stop if open-nwe status changes after read-only smoke.
- Stop if installed manifest and runtime initialize disagree after promotion.

## Implementation Approach

1. Add MCP JSON-RPC regression tests that currently fail:
   - all four facades accept `job_status` and `job_detail` without `root`
   - invalid `jobId` returns structured job error rather than root validation
   - workspace `mode=job` compact response omits full project details and `job_detail` provides paged schema
2. Move job mode dispatch ahead of root validation for the four facade handlers.
3. Split job summaries into compact summaries and detail pages; workspace project arrays go only into `job_detail`.
4. Strengthen busy guidance to explain recovery and recommend job mode for large workspaces.
5. Replace the installed smoke with a session-based JSON-RPC smoke that performs initialize, initialized notification, tools/list, and tools/call in the same process.
6. Build with all language features and promote to `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
7. Run the required regression suite, native governance, installed wrapper self-test, and open-nwe read-only smoke if the directory exists.
