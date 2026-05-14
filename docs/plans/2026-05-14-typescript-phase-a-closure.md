# TypeScript Phase A — Closure

> Date: 2026-05-14
> Status: COMPLETE
> Commit: (pending)

## Results

- Feature wiring: `tree-sitter-typescript` independent of `tree-sitter-arkts`
- `find_typescript_project_root()` — ignores oh-package.json5
- Fixtures: `portable-smoke` (4 .ts files, 20 symbols, 54 edges) + `tsx-smoke`
- Graph schema: `label` as type classifier, `kind` as kebab-case, edge `type` as SCREAMING_SNAKE_CASE
- Bridge output: properties-based name/path extraction (backward compatible)
- MCP tests: 7 new tests (12001-12007), cfg-gated behind `tree-sitter-typescript`
- Smoke: synthetic 6-file project, 4/4 assertions pass
- Total tests: 90 (83 original + 7 TS MCP), default 83, ArkTS 89

## Files Changed

- `crates/cli/Cargo.toml` — added `tree-sitter-typescript` feature
- `crates/typescript/src/lib.rs` — export `find_typescript_project_root`
- `crates/typescript/src/project.rs` — new `find_typescript_project_root()`
- `crates/typescript/src/graph.rs` — label/kind/edge schema alignment
- `crates/cli/src/lib.rs` — cfg gate fix, summary builder normalization
- `crates/cli/src/arkts_bridge.rs` — bridge name/path extraction from properties
- `crates/cli/src/mcp_server.rs` — TypeScript language feature check
- `crates/cli/tests/mcp_server.rs` — 7 new TS MCP tests
- `fixtures/typescript/portable-smoke/` — enhanced with math.ts, model.ts
- `fixtures/typescript/tsx-smoke/` — new TSX fixture
- `scripts/typescript-real-project-smoke.sh` — new
- `docs/architecture/mcp-v0-contract.md` — v0.12.0, TypeScript section
