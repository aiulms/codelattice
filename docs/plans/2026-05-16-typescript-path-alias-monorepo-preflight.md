# TypeScript Path Alias / Monorepo Support — Preflight / Execution Card

**Created**: 2026-05-16
**Status**: In Progress
**Depends on**: `bc17b03 feat(python): refine package import resolution`

## Scope

### Supported
- tsconfig.json `compilerOptions.baseUrl`
- tsconfig.json `compilerOptions.paths` (exact + wildcard `*`)
- tsconfig `extends` chain (relative path, implicit `.json` extension)
- Monorepo workspace package import (via root package.json `workspaces`)
- `@/xxx`, `@core/xxx`, `@pkg/shared` style imports
- `index.ts` / `index.tsx` / `.d.ts` extensionless resolution
- `.ts` / `.tsx` / `.d.ts` extension resolution
- Import alias map for call resolution enhancement
- Confidence tiers: relative 0.90, exact alias 0.90, wildcard 0.85, workspace 0.80, workspace subpath 0.75
- Diagnostics for unresolved, external, star imports

### Not Supported
- No `node_modules` reading
- No TypeScript compiler API usage
- No type inference
- No `package.json` `exports` condition resolution (full)
- No URL / remote import resolution
- No dynamic imports (`import()`) resolution
- No `export *` expansion (diagnostic only)
- No JSONC trailing comma support in tsconfig (unless existing `strip_json5_comments` handles it)
- No `references` / `composite` project references
- No `paths` with multiple fallback patterns (use first match only)

## Strategy

### tsconfig Resolution
1. Discover tsconfig.json / tsconfig.*.json from project root and subdirectories
2. Parse each with `strip_json5_comments` for JSONC support
3. Resolve `extends` chains: relative path, implicit `.json` extension
4. Merge: child `compilerOptions` overrides parent; `paths` merged with child overriding same key
5. `baseUrl` resolved relative to the tsconfig file's directory

### Path Alias Resolution
1. Exact match: `@shared` → check paths `@shared` → resolve target
2. Wildcard match: `@core/logger` → check paths `@core/*` → replace `*` with `logger` → resolve target
3. Resolution tries: exact path → `.ts` → `.tsx` → `.d.ts` → `/index.ts` → `/index.tsx` → `/index.d.ts`

### Workspace Package Resolution
1. Read root package.json `workspaces` array
2. For each workspace pattern, discover sub-package directories
3. Read each sub-package's package.json for `name`
4. Map `name` → package root → resolve `src/index.ts` / `index.ts` as entry
5. Subpath: `@pkg/shared/format` → package `@pkg/shared` → `src/format.ts`

### External Package Detection
1. Specifier not matching any path alias or workspace package
2. Not a relative path (doesn't start with `.` or `/`)
3. Mark as external — no edge, diagnostic only

## Edge Confidence / Reason Strategy

| Resolution Kind | Confidence | Reason |
|---|---|---|
| Relative import resolved | 0.90 | `typescript-relative-import-resolved` |
| tsconfig paths exact match | 0.90 | `typescript-tsconfig-path-exact` |
| tsconfig paths wildcard | 0.85 | `typescript-tsconfig-path-wildcard` |
| Workspace package root | 0.80 | `typescript-workspace-package-import` |
| Workspace package subpath | 0.75 | `typescript-workspace-subpath-import` |
| External package (not indexed) | N/A | `typescript-external-package-not-indexed` |
| Unresolved | N/A | `typescript-import-unresolved` |
| Star import (not expanded) | N/A | `typescript-star-import-not-expanded` |

## Write Set

### New Files
- `crates/typescript/src/tsconfig.rs` — tsconfig parsing, extends, baseUrl/paths extraction
- `crates/typescript/src/module_resolution.rs` — specifier → file resolution
- `crates/typescript/tests/path_alias_resolution.rs` — crate-level tests
- `fixtures/typescript/path-alias-monorepo/` — 16-file monorepo fixture

### Modified Files
- `crates/typescript/src/lib.rs` — expose new modules
- `crates/typescript/src/graph.rs` — import edges use resolver, diagnostics field
- `crates/typescript/src/manifest.rs` — make `strip_json5_comments` pub
- `crates/cli/src/lib.rs` — build TsModuleResolver, pass to build_ts_graph
- `crates/cli/tests/mcp_server.rs` — new path alias MCP tests
- `CHANGELOG.md` — Unreleased entry

### Forbidden Set
- No GitNexus-RC / GitNexus-RC-Tool modifications
- No `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` modifications
- No Codex/opencode/Claude config modifications
- No new dependencies in Cargo.toml
- No `node_modules` reading
- No dangling edges
- No confidence inflation
- No C/C++ include path work
- No Python work
- No WebUI

## Stop-Line
- If baseline tests fail → stop and report
- If cargo fmt/check fails → fix before continuing
- If Oracle recommends fundamentally different architecture → reassess before implementing

## Verification Plan
1. `cargo fmt --check`
2. `git diff --check`
3. `cargo test -p gitnexus-typescript --features tree-sitter-typescript` (crate tests)
4. `cargo test --test mcp_server --features tree-sitter-arkts` (MCP tests including TS)
5. `cargo test --all-features`
6. `python3 scripts/real-project-corpus-smoke-test.py`
7. Tool detect-changes
8. Manual fixture verification with analyze command

## Expected Impact on Quality Metrics
- `importEdgeCount` should increase (more imports resolved to real targets)
- `danglingEdgeCount` should stay 0 or decrease
- `unresolvedImportOrIncludeCount` may change (external imports now counted)
- Overall confidence distribution should shift higher (more resolved imports)

## Architecture Decision
- Follow Python import resolution pattern: new `module_resolution.rs` with `TsModuleResolver`, built in CLI pipeline, passed as `Option<&TsModuleResolver>` to `build_ts_graph`
- Separate `tsconfig.rs` for tsconfig parsing concerns
- Backward compat: when module resolver is None, use old `module:{specifier}` synthetic targets
