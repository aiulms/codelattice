# CodeLattice Smoke Matrix

> **Platform**: macOS (`darwin-arm64`), Apple Silicon
> **Date**: 2026-05-15
> **Version**: 0.13.0-beta.2
> **Rust**: stable (via `rustc`)

## Feature Combinations

| Features | CLI | MCP | Notes |
|----------|-----|-----|-------|
| default (`tree-sitter-extraction`) | ✅ | ✅ | Rust-only, always available |
| `+tree-sitter-cangjie` | ✅ | ✅ | Cangjie / 仓颉 symbol + call analysis |
| `+tree-sitter-arkts` | ✅ | ✅ | ArkTS / HarmonyOS component extraction |
| `+tree-sitter-typescript` | ✅ | ✅ | TypeScript symbol + import + call analysis |
| `--all-features` | ✅ | ✅ | All languages enabled |

## MCP Tool Count

22 tools (with all features enabled).

## Fixtures

### Rust (stable)

| Fixture | Path | What it tests |
|---------|------|---------------|
| portable-smoke | `fixtures/rust/portable-smoke` | Full graph output, quality gates, MCP tools |
| call-resolution (c1–c16) | `fixtures/call-resolution/c*` | Import binding, crate/self/super paths, associated functions, enum constructors, receiver methods, cross-file calls, wildcard disambiguation |
| import-use | `fixtures/import-use/*` | use statements, aliases, groups, self/super, re-exports |
| item-extraction | `fixtures/item-extraction/*` | Function, struct, enum, trait, impl, const, static, macro extraction |
| source-ownership | `fixtures/source-ownership/*` | Package/workspace/target ownership, virtual workspace |
| enum-variant | `fixtures/rust/enum-variant` | Enum variant constructor resolution |
| imports-cross-crate | `fixtures/rust/imports-cross-crate` | Cross-crate import handling |
| module-hierarchy | `fixtures/rust/module-hierarchy` | Module nesting |
| workspace-member | `fixtures/rust/workspace-member` | Workspace with multiple members |

### Cangjie / 仓颉 (stable)

| Fixture | Path | What it tests |
|---------|------|---------------|
| portable-smoke | `fixtures/cangjie/portable-smoke` | Full graph, quality gates, MCP tools |
| cjpm-basic | `fixtures/cangjie/cjpm-basic` | Basic package |
| cjpm-workspace | `fixtures/cangjie/cjpm-workspace` | Workspace with pkg1, pkg2 |
| imports-basic | `fixtures/cangjie/imports-basic` | Named/alias/wildcard imports |
| reference-cross-file-basic | `fixtures/cangjie/reference-cross-file-basic` | Cross-file references |
| reference-function-call-cross-file | `fixtures/cangjie/reference-function-call-cross-file` | Cross-file function call references |
| constructor-basic | `fixtures/cangjie/constructor-basic` | Init/constructor extraction |
| constructor-cross-file | `fixtures/cangjie/constructor-cross-file` | Cross-file constructors |

### ArkTS / HarmonyOS (production trial)

| Fixture | Path | What it tests |
|---------|------|---------------|
| portable-smoke | `fixtures/arkts/portable-smoke` | Component, buildMethod, @State, UI call extraction |
| cross-file | `fixtures/arkts/cross-file` | Cross-file import edges |

### TypeScript (Phase A)

| Fixture | Path | What it tests |
|---------|------|---------------|
| portable-smoke | `fixtures/typescript/portable-smoke` | Functions, interfaces, type aliases, imports, calls |
| tsx-smoke | `fixtures/typescript/tsx-smoke` | TSX/JSX component extraction |

## Smoke Scripts

### Release Gate (must pass before release)

| Script | What it does | Required |
|--------|-------------|----------|
| `cargo fmt --check` | Code formatting | ✅ |
| `git diff --check` | No whitespace issues | ✅ |
| `cargo test --test mcp_server` | 89 MCP integration tests | ✅ |
| `cargo test` | All unit tests | ✅ |
| `cargo test --all-features` | Combined optional adapter tests | ✅ |
| `scripts/mcp-dogfood.sh` | 22-tool MCP walkthrough | ✅ |
| `scripts/mcp-cache-smoke.sh` | Cache hit/miss/persistent (6 tests) | ✅ |
| `scripts/package-release.sh` | Build all-language tarball + manifest | ✅ |
| `scripts/release-smoke.sh` | Tarball unpack + Rust/Cangjie/ArkTS/TypeScript verify | ✅ |
| `scripts/fresh-clone-smoke.sh` | Simulated external clone path | ✅ |
| `scripts/linux-source-build-smoke.sh` | Source-build platform preflight | ✅ |

### Feature-Optional (run when feature flag available)

| Script | Feature | What it does |
|--------|---------|-------------|
| `cargo test --features tree-sitter-cangjie` | Cangjie | Cangjie adapter tests |
| `cargo test --features tree-sitter-arkts` | ArkTS | ArkTS adapter tests |
| `cargo test --features tree-sitter-typescript` | TypeScript | TypeScript adapter tests |
| `cargo test --all-features` | All | Combined feature test |

### Local-Only (not required for release, developer convenience)

| Script | What it does |
|--------|-------------|
| `scripts/mcp-local-client-smoke.sh` | Local MCP client connection test |
| `scripts/mcp-real-client-dry-run.sh` | Dry-run AI client config |
| `scripts/cangjie-live-codelattice-smoke.sh` | Cangjie live project analysis |
| `scripts/typescript-real-project-smoke.sh` | TypeScript real project analysis |
| `python3 scripts/real-project-corpus-smoke.py` | GitCode real-project corpus smoke for C / C++ / Python defaults |
| `scripts/alpha-trial-smoke.sh` | Alpha trial validation |
| `scripts/linux-source-build-smoke.sh --all-language-features` | Linux / openEuler source-build compatibility smoke |

### Real Project Production Trials (local, non-distributable)

| Project | Language | Status |
|---------|----------|--------|
| CoolMallArkTS | ArkTS | ✅ Production trial passed (local only) |
| harmony-utils | ArkTS | ✅ Production trial passed (local only) |
| HarmonyOS-Examples | ArkTS | ✅ Production trial passed (local only) |

These projects are referenced only as local verification targets. They are not included in the release tarball.

## Test Counts

| Suite | Count | Status |
|-------|-------|--------|
| MCP integration tests | 89 | ✅ Pass |
| Project model tests | 93 | ✅ Pass |
| Cache smoke tests | 6/6 | ✅ Pass |
| Dogfood tests | 22/22 | ✅ Pass |
| Fresh clone smoke | ✅ | ✅ Pass |
| Release smoke | ✅ | ✅ Pass |
