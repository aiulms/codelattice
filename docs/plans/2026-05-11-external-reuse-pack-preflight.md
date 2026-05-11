# External Reuse Pack Preflight

Date: 2026-05-11
Status: Active

## Goal

Move CodeLattice from "works on this machine" to a fresh-clone-friendly alpha package:
portable MCP install scripts, a fresh clone smoke harness, external-facing docs, and one
small MCP compact-count regression fix.

## Truth Gate

- Repo path: `/Users/jiangxuanyang/Desktop/codelattice`
- HEAD at preflight: `2dcd371403a805b9d656bd6c435bf0f57779c7a6`
- Indexed GitNexus repo: `codelattice`
- Working tree at preflight: only agent-private untracked directories (`.agents/`, `.claude/`)
- Stable runtime baseline: `sourceCommit=2dcd371`, `serverVersion=0.7.0`, `cangjieSupport=true`, `toolCount=21`

## Baseline Commands

All baseline commands were run before edits:

```bash
cargo fmt --check
git diff --check
cargo test --test mcp_server
scripts/install-mcp.sh --doctor
scripts/codelattice-mcp.sh --self-test
```

Observed result:

- `cargo fmt --check`: pass
- `git diff --check`: pass
- `cargo test --test mcp_server`: 51 passed, 0 failed; existing warnings in `mcp_server.rs`
- `scripts/install-mcp.sh --doctor`: 7 passed, 0 failed
- `scripts/codelattice-mcp.sh --self-test`: pass, 21 tools, Cangjie support enabled

## Impact Gate

GitNexus impact was run before editing `handle_project_overview`:

- Target: `handle_project_overview`
- Direct caller: `handle_request`
- Affected flows: `handle_request`, `run_mcp_server`
- Risk: LOW

No HIGH or CRITICAL warning was returned.

## Write Set

- `scripts/install-mcp.sh`
- `scripts/promote-to-local-tool.sh`
- `scripts/codelattice-mcp.sh`
- `scripts/fresh-clone-smoke.sh`
- `README.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-11-external-reuse-pack-preflight.md`
- `docs/plans/2026-05-11-external-reuse-pack-closure.md`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`

## Forbidden Set

- No GitNexus-RC runtime, adapter, schema, WebUI, or package changes.
- No GitNexus-RC-Tool changes.
- No `/Users/jiangxuanyang/Desktop/cangjie` live repo changes.
- No `open-nwe` changes.
- No Codex/opencode/Claude real client config writes.
- No promote to `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` unless explicitly requested after this round.
- No Cargo package/bin rename.
- No new dependencies unless a hard blocker appears.

## Stop-Line

This pack is packaging, docs, and smoke hardening only. It does not expand language
semantics, change graph schema, make CodeLattice a GitNexus-RC replacement, or switch
any local AI client default.

## Execution Plan

1. Make MCP install/config scripts derive paths from `CODELATTICE_ROOT`, script location,
   `CODELATTICE_TOOL_DIR`, or `--install-dir`.
2. Keep stable runtime self-contained after promotion; generated wrapper must not depend on
   the development checkout.
3. Add `scripts/fresh-clone-smoke.sh` to copy the current repo into `/tmp`, exclude local
   artifacts, build/install/promote to a temp runtime, run MCP self-test/tools/list, and
   analyze portable fixtures without touching real client configs.
4. Externalize README and MCP setup docs for clone/install/promote/client configuration.
5. Reproduce the Cangjie `project_overview` compact count regression, add a feature-gated
   MCP regression test, then make the smallest handler fix.
6. Run the requested verification matrix, refresh the Tool index if changes are material,
   then commit and push if all gates pass.

## Known Regression Reproduction

Current `codelattice_project_overview` on `fixtures/cangjie/portable-smoke` returns:

```json
{
  "nodeCount": 27,
  "edgeCount": 0,
  "symbolCount": 0,
  "sourceFileCount": 0,
  "packageCount": 0
}
```

The same fixture through `codelattice_summary` and `codelattice_graph_overview` reports
nonzero counts. Initial root-cause evidence: `project_overview` derives some counts from
`GraphView` helpers that assume Rust graph fields (`label`, `source`, `target`, `type`),
while Cangjie graph output uses `kind`, `sourceId`, `targetId`, and edge `kind`.
