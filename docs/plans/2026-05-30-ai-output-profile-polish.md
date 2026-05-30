# AI Output Profile Polish Plan

Goal: tighten CodeLattice's AI-facing CLI output so large projects can be explored without forcing full graph payloads, and make workspace project counts reflect manifest-backed boundaries.

Execution card:
- Write set: `crates/cli/src/lib.rs`, `crates/workspace-model/src/lib.rs`, `crates/cli/tests/productization_commands.rs`, `crates/cli/tests/mcp_server.rs`, docs under `docs/`.
- Forbidden set: live repositories such as `open-nwe`, installed `CodeLattice-Tool` until verification passes.
- Stop-line: do not change MCP tool counts; default AI toolset must remain 6 and full toolset 49.

Tasks:
1. Add bounded profile controls to `codelattice analyze`: page/page-size/public-only for `symbols` and `modules`; keep `full` unchanged.
2. Add paging metadata and `detailHint` to bounded profile outputs so AI can request the next slice without guessing.
3. Update workspace graph construction so source-only directories are `source_area` nodes, not counted as manifest-backed projects.
4. Add red/green tests for profile paging, public-only filtering, source-only workspace counting, and clean compact output.
5. Run formatting, tests, native precommit, sync installed tool only after all checks pass.
