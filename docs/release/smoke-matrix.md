# CodeLattice Smoke Matrix

> **Platform**: macOS (`darwin-arm64`), Apple Silicon
> **Date**: 2026-06-05
> **Version**: 0.17.0-beta.1 release candidate on master
> **Rust**: stable (via `rustc`)

## Feature Combinations

| Features | CLI | MCP | Notes |
|----------|-----|-----|-------|
| default (`tree-sitter-extraction`) | ✅ | ✅ | Rust-only baseline |
| `+tree-sitter-cangjie` | ✅ | ✅ | Cangjie / 仓颉 symbol + call analysis |
| `+tree-sitter-arkts` | ✅ | ✅ | ArkTS / HarmonyOS component extraction |
| `+tree-sitter-typescript` | ✅ | ✅ | TypeScript symbols, imports, calls, tsconfig path aliases |
| `+tree-sitter-javascript` | ✅ | ✅ | JavaScript/JSX/MJS/CJS symbols, imports, calls, package entry points |
| `+tree-sitter-c` | ✅ | ✅ | C symbols, includes, compile_commands include paths |
| `+tree-sitter-cpp` | ✅ | ✅ | C++ symbols, includes, calls, compile_commands include paths |
| `+tree-sitter-python` | ✅ | ✅ | Python symbols, calls, package import resolution |
| Shell static analyzer | ✅ | ✅ | Shell is built into the CLI; no optional parser feature |
| full beta release features | ✅ | ✅ | All nine supported language paths available |

Full beta release feature string:

```text
tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-javascript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python
```

## MCP Tool Count

Default AI toolset: 6 facade-first entry tools.

Full debug/regression toolset: 49 tools with all language features enabled.

The release gate requires:

- default `tools/list == 6`
- `CODELATTICE_MCP_TOOLSET=full tools/list >= 49`
- `initialize.serverInfo.cangjieSupport == true`
- `initialize.serverInfo.arktsSupport == true`
- `initialize.serverInfo.typescriptSupport == true`
- `initialize.serverInfo.javascriptSupport == true`
- `initialize.serverInfo.cSupport == true`
- `initialize.serverInfo.cppSupport == true`
- `initialize.serverInfo.pythonSupport == true`
- `initialize.serverInfo.shellSupport == true`

## Language Fixtures

| Language | Fixture | Path | Release smoke |
|----------|---------|------|---------------|
| Rust | portable-smoke | `fixtures/rust/portable-smoke` | ✅ |
| Cangjie / 仓颉 | portable-smoke | `fixtures/cangjie/portable-smoke` | ✅ |
| ArkTS / HarmonyOS | portable-smoke | `fixtures/arkts/portable-smoke` | ✅ |
| TypeScript | portable-smoke | `fixtures/typescript/portable-smoke` | ✅ |
| JavaScript | portable-smoke | `fixtures/javascript/portable-smoke` | ✅ |
| C | portable-smoke | `fixtures/c/portable-smoke` | ✅ |
| C++ | portable-smoke | `fixtures/cpp/portable-smoke` | ✅ |
| Python | portable-smoke | `fixtures/python/portable-smoke` | ✅ |
| Shell | portable-smoke | `fixtures/shell/portable-smoke` | ✅ |

## Extended Fixtures

| Language | Fixture | What it tests |
|----------|---------|---------------|
| Rust | `fixtures/call-resolution/c*` | Import binding, crate/self/super paths, associated functions, enum constructors, receiver methods, cross-file calls, wildcard disambiguation |
| Rust | `fixtures/import-use/*` | `use` statements, aliases, groups, self/super, re-exports |
| Rust | `fixtures/item-extraction/*` | Function, struct, enum, trait, impl, const, static, macro extraction |
| Rust | `fixtures/source-ownership/*` | Package/workspace/target ownership |
| Cangjie | `fixtures/cangjie/imports-basic` | Named/alias/wildcard imports |
| Cangjie | `fixtures/cangjie/reference-cross-file-basic` | Cross-file references |
| Cangjie | `fixtures/cangjie/constructor-basic` | Init/constructor extraction |
| ArkTS | `fixtures/arkts/cross-file` | Cross-file import edges |
| TypeScript | `fixtures/typescript/path-alias-monorepo` | tsconfig paths, workspace packages, extensionless imports |
| TypeScript | `fixtures/typescript/tsx-smoke` | TSX/JSX component extraction |
| C | `fixtures/c/include-compile-commands` | compile_commands include path resolution |
| C++ | `fixtures/cpp/include-compile-commands` | compile_commands include path resolution and graph endpoint integrity |
| Python | `fixtures/python/import-resolution` | package imports, relative imports, aliases, re-exports |

## Release Gate

| Command | Purpose | Required |
|---------|---------|----------|
| `cargo fmt --check` | Formatting | ✅ |
| `git diff --check` | Whitespace and patch hygiene | ✅ |
| `cargo test --test mcp_server` | Default MCP integration suite | ✅ |
| `cargo test` | Default unit/integration/doc tests | ✅ |
| `cargo test --all-features` | Full optional adapter suite | ✅ |
| `python3 scripts/real-project-corpus-smoke-test.py` | Real corpus harness unit tests | ✅ |
| `scripts/codelattice-mcp.sh --self-test` | Wrapper self-test and language support profile | ✅ |
| `scripts/mcp-dogfood.sh` | Full-toolset MCP walkthrough | ✅ |
| `scripts/install-mcp.sh --doctor` | Local install doctor | ✅ |
| `scripts/package-release.sh` | Build full-language tarball and manifest | ✅ |
| `scripts/release-smoke.sh --tarball <tarball>` | Tarball unpack + nine-path fixture smoke | ✅ |
| `scripts/fresh-clone-smoke.sh --skip-tests` | Simulated external clone/install without full tests | ✅ |
| `scripts/fresh-clone-smoke.sh` | Full simulated external clone/install | ✅ |

## Real Project Baseline

Default beta evidence targets:

| Target | Language | Baseline |
|--------|----------|----------|
| redis-c | C | ✅ |
| catch2-cpp | C++ | ✅ |
| pip-python | Python | ✅ |

Optional broader targets may be used when cached locally:

| Target | Language |
|--------|----------|
| vite-typescript | TypeScript |
| codelattice-rust | Rust |
| cangjie-magic | Cangjie |
| openharmony-app-samples-arkts | ArkTS |

The real-project corpus is a smoke/baseline signal, not a precision proof. It does not vendor target repositories and does not run target project builds.

## Current Stage 0 Baseline

Recorded on 2026-05-16 before release docs/package edits:

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | ✅ Pass |
| `git diff --check` | ✅ Pass |
| `cargo test --test mcp_server` | ✅ 104/104 |
| `cargo test` | ✅ Pass |
| `cargo test --all-features` | ✅ Pass |
| `python3 scripts/real-project-corpus-smoke-test.py` | ✅ 10/10 |
| `scripts/codelattice-mcp.sh --self-test` | ✅ 49 full tools, all language flags true |
| `scripts/mcp-dogfood.sh` | ✅ 49-tool full profile smoke |

`v0.17.0-beta.1` is the current release target and includes JavaScript, Shell, default six-tool AI MCP surface, workspace intelligence, and native change review.
