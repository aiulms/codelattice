# TypeScript Phase A — Preflight

> Date: 2026-05-14
> Status: COMPLETE
> Commit: (pending)

## Goal

TypeScript (`.ts`/`.tsx`) Phase A: local code graph analysis through CLI and MCP.

## Scope

- Separate `tree-sitter-typescript` feature from `tree-sitter-arkts`
- Enhanced portable fixtures (4 .ts files + TSX)
- Graph schema alignment with Rust/ArkTS conventions
- CLI `--language typescript` with `--format json` and `--format gitnexus-rc`
- MCP 7-tool TypeScript closure
- Synthetic project smoke script

## Non-Goals

- No npm/tsc execution
- No type checking / type inference
- No tsserver replacement
- No node_modules / path alias / monorepo resolution
- No JSX framework semantics
