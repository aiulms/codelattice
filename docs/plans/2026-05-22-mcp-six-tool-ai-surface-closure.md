# MCP Six-Tool AI Surface Closure

Date: 2026-05-22

## Result

Default external MCP toolset is now exactly 6 tools:

1. `codelattice_workflow`
2. `codelattice_project`
3. `codelattice_symbol`
4. `codelattice_change_review`
5. `codelattice_workspace`
6. `codelattice_cache`

The full expert/debug surface remains available through `CODELATTICE_MCP_TOOLSET=full` and still reports 49 tools. The core profile remains available for advanced agents and currently reports 25 tools.

## What Changed

- Reduced `AI_TOOLSET_TOOLS` to six intent-oriented entry tools.
- Ordered default `tools/list` so `codelattice_workflow` appears first.
- Moved AI context into `codelattice_project mode=ai_context`.
- Moved cleanup/release/root-cause paths into `codelattice_change_review` modes:
  - `safe_cleanup_review`
  - `dead_code`
  - `reachability`
  - `external_api`
  - `framework_entries`
  - `release_check`
  - `docs_tests`
  - `config_examples`
  - `root_cause`
- Updated `codelattice_workflow` next actions so default AI clients are not routed to hidden tools.
- Added `docs/guides/ai-mcp-tool-guide.md` as the short AI-facing selection manual.
- Updated README, MCP setup docs, MCP contract docs, and facade smoke expectations.

## Verification

- `cargo fmt --check`: PASS
- `git diff --check`: PASS
- `cargo test --test mcp_server`: 139/139 PASS
- `bash scripts/codelattice-mcp-facade-smoke.sh`: 13/13 PASS
- `scripts/codelattice-precommit-check.sh`: PASS
- `cargo build --release --all-features --bin codelattice`: PASS
- default `tools/list`: `ai 6 6`
- promoted `/Users/jiangxuanyang/Desktop/CodeLattice-Tool/codelattice-mcp.sh` default `tools/list`: `ai 6 6`

Native precommit `detect-changes` reported `critical` because this change touches the MCP routing core and cross-project graph reports CLI/language/script downstream impact. The risk was reviewed before commit; regression checks above passed.

## Boundary

- No existing MCP tool was deleted.
- No `core` / `full` specialist capability was removed.
- No GitNexus-RC, GitNexus-RC-Tool, AI client config, or real project source was modified.
- CodeLattice-Tool runtime was updated through the existing promote script only.

