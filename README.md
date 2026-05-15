# CodeLattice

CodeLattice is a local code analyzer and AI coding sidecar. It analyzes Rust, Cangjie / 仓颉, ArkTS / HarmonyOS, and TypeScript projects to extract symbols, call relationships, documentation associations, and impact risks. It provides these capabilities through CLI and MCP sidecar for AI programming assistants, code review, and local engineering tools.

CodeLattice runs entirely on your local machine and never uploads code. It performs read-only analysis by default and exposes capabilities via stdio JSON-RPC.

**Current Status: External Beta (`v0.13.0-beta.1`)** — Local production trial passed, not GA release. See [CHANGELOG](CHANGELOG.md) and [Smoke Matrix](docs/release/smoke-matrix.md).

## Who Is It For

- Developers who want AI agents to understand code structure before answering, modifying, or performing impact analysis.
- Teams that need to generate code graphs, symbol indexes, call relationships, and quality gate reports locally.
- Developers maintaining Rust or Cangjie projects who want a scriptable, smoke-testable, MCP-client-connectable local analysis core.
- Users who want to embed code understanding capabilities into their own toolchain without needing a WebUI or hosted platform.

## Core Capabilities

| Capability | Description |
|-----------|-------------|
| Project Model | Identifies Rust Cargo projects, Cangjie cjpm projects, and ArkTS HarmonyOS projects; establishes package, target, and source ownership |
| Symbol Indexing | Extracts functions, methods, types, traits/interfaces, enums, macros, init, and other language symbols; ArkTS additionally extracts @Component, @State, build() |
| Call Resolution | Resolves same-module, cross-file, import binding, partial associated functions, and limited receiver methods |
| Graph Output | Outputs repository / package / source file / symbol / diagnostic nodes and relationship edges; ArkTS additionally outputs component / buildMethod / UI call nodes |
| Quality Gates | Checks dangling edges, duplicates, statistical consistency, stdout JSON purity, deterministic output |
| MCP Sidecar | Provides 22 MCP tools supporting AI client queries for project overview, symbol context, call relationships, impact preview, change detection, and documentation association |
| Persistent Cache | Two-layer cache (memory + disk) with fingerprint invalidation detection for cross-process analysis result reuse |
| Local Security | Read-only by default; wrapper and stable runtime can be isolated; configuration scripts only print templates without writing real client configurations |

## Quick Start

### 1. Clone and Build

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

# Build release binary with default Rust + Cangjie support
bash scripts/install-mcp.sh --build
```

After building, use the public binary:

```bash
target/release/codelattice --version
```

Compatibility note: The Cargo package is still named `gitnexus-rust-core-cli` and continues to build a compatibility binary with the same name; the external command name `codelattice` is preferred.

### 2. Run Fresh Clone Smoke Test

```bash
bash scripts/fresh-clone-smoke.sh --skip-tests
```

This script copies the current repo to `/tmp/codelattice-fresh-smoke-*` to simulate an external fresh clone without network cloning and without touching real AI client configurations. By default, it validates the build, temporary stable runtime installation, MCP wrapper self-test, tools/list, Rust fixture, and Cangjie fixture when available.

For full testing:

```bash
bash scripts/fresh-clone-smoke.sh
```

### 3. Install Stable MCP Runtime

AI clients should point to the promoted stable wrapper rather than scripts in the development checkout.

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

If `--install-dir` is not provided, the script uses the default directory `$HOME/Desktop/CodeLattice-Tool`.

### 4. Analyze a Rust Fixture

```bash
target/release/codelattice analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

### 5. Install from Release Tarball

Current `v0.13.0-beta.1` has published macOS Apple Silicon (`darwin-arm64`) release packages:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.13.0-beta.1 \
  --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

This installer downloads the GitCode Release tarball, verifies `.sha256`, installs the stable MCP wrapper, and runs self-test. It does not modify Codex / opencode / Claude configurations. See [Install Guide](docs/release-install.md) and [Upgrade Guide](docs/release/upgrade.md).

Linux and other platforms currently follow the source build path; multi-platform release artifacts are the next packaging milestone. See the [Linux / openEuler source build guide](docs/platforms/linux-openeuler.md) for prerequisites and smoke commands.

### 6. Print MCP Client Configuration Template

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

This command only prints Codex / opencode / Claude configuration snippets without modifying any real configuration. The wrapper in the configuration should point to:

```text
$CODELATTICE_TOOL_DIR/codelattice-mcp.sh
```

### 7. Package Release Tarball

```bash
bash scripts/check-release-metadata.sh
bash scripts/package-release.sh
bash scripts/release-smoke.sh
```

Default artifacts:

```text
dist/codelattice-<version>-<platform>.tar.gz
dist/codelattice-<version>-<platform>.tar.gz.sha256
```

Version rules are in `docs/release-versioning.md`; release notes are in `CHANGELOG.md`. The product version comes from Cargo `workspace.package.version`; the MCP `serverVersion` is the sidecar tool/profile version, managed separately.

## Supported Languages

| Language | Status | Feature Flag |
|----------|--------|-------------|
| Rust | **Stable** | `tree-sitter-extraction` (enabled by default) |
| Cangjie / 仓颉 | **Stable** | `tree-sitter-cangjie` |
| ArkTS / HarmonyOS | **Production Trial** | `tree-sitter-arkts` |
| TypeScript | **Phase A** | `tree-sitter-typescript` |

## CLI Usage

### Analyze Rust Project

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json
```

Strict quality gates:

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json \
  --strict
```

### Analyze Cangjie / 仓颉 Project

```bash
target/release/codelattice analyze \
  --root /path/to/cangjie/project \
  --language cangjie \
  --format json \
  --strict
```

### Analyze ArkTS / HarmonyOS Project

```bash
target/release/codelattice analyze \
  --root /path/to/arkts/project \
  --language arkts \
  --format json
```

Bridge format output (for GitNexus-RC consumption):

```bash
target/release/codelattice analyze \
  --root /path/to/arkts/project \
  --language arkts \
  --format gitnexus-rc
```

> **Alpha Status:** ArkTS support is currently in production trial phase. Known limitations:
> - The `struct` keyword is parsed as an ERROR node by tree-sitter-typescript; component definitions are recovered through pattern matching
> - Cross-file references are resolved as import edges (`module:../path`) without symbol-level cross-file binding
> - Does not analyze advanced decorators such as `@Builder`, `@Extend`
> - Does not parse ArkUI declarative syntax trees in `.ets` files (only extracts UI call names)
> - Requires compilation with `--features tree-sitter-arkts` to enable

### Auto-Detect Language

```bash
target/release/codelattice analyze \
  --root /path/to/project \
  --language auto \
  --format json
```

Auto-detection rules:

- Finds `Cargo.toml`: Rust
- Finds `cjpm.toml`: Cangjie / 仓颉
- Finds `oh-package.json5`: ArkTS
- Multiple detected simultaneously: requires explicit `--language`

### Quality Gate Check

```bash
target/release/codelattice quality \
  --root fixtures/rust/portable-smoke \
  --language rust
```

Exit codes:

- `0`: Quality gates passed
- `1`: Quality gates failed
- `2`: Project language or structure unclear

### Summary Output

```bash
target/release/codelattice summary \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

## MCP Sidecar

CodeLattice provides an MCP server via JSON-RPC over stdio, callable by MCP-enabled clients such as Codex, opencode, and Claude Desktop.

For development debugging, you can use the checkout wrapper directly:

```bash
bash scripts/codelattice-mcp.sh --self-test
```

For daily AI client use, promote first:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `codelattice_analyze` | Analyze project, return graph summary + quality gates |
| `codelattice_quality` | Quality gate check, failed gates listed first |
| `codelattice_summary` | Compact summary with stats + quality, no graph |
| `codelattice_smoke` | End-to-end smoke test |
| `codelattice_graph_overview` | Graph scale overview with node/edge/symbol counts and kind breakdowns |
| `codelattice_unresolved_report` | Unresolved call report |
| `codelattice_symbol_search` | Search symbols by name |
| `codelattice_export_bridge` | Export bridge JSON to `/tmp` for downstream graph consumption |
| `codelattice_symbol_context` | Symbol context, call relationship summary, and source snippets |
| `codelattice_calls_from` | Query what a symbol calls outward |
| `codelattice_calls_to` | Query which symbols call the target symbol |
| `codelattice_impact_preview` | Read-only impact preview providing risk level, risk reasons, impact metrics, confidence summary, and review focus |
| `codelattice_query_graph` | Parameterized local graph query |
| `codelattice_project_overview` | Project-level overview, suitable for AI quick modeling |
| `codelattice_repo_registry` | Read-only repo registry/status view |
| `codelattice_rename_preview` | Rename preview, read-only, does not write files |
| `codelattice_cache_status` | View process-local cache status (memory + persistent dual-layer) |
| `codelattice_cache_clear` | Clear cache, supports memory / persistent / both layer selection |
| `codelattice_production_assist` | One-stop summary: quality gates + unresolved calls + diagnostics + change risk + review checklist; auto-detects changed symbols from git diff |
| `codelattice_compare_runs` | Compare node and edge changes between two bridge/run artifacts |
| `codelattice_cache_prewarm` | Prewarm process-local cache to improve real client first-interaction experience |
| `codelattice_changed_symbols` | Auto-detect changed symbols from git diff, mapping hunks to graph symbols |

Common validations:

```bash
bash scripts/install-mcp.sh --doctor
bash scripts/codelattice-mcp.sh --self-test
bash scripts/check-release-metadata.sh
bash scripts/mcp-dogfood.sh
bash scripts/mcp-local-client-smoke.sh
bash scripts/mcp-real-client-dry-run.sh
```

### AI Sidecar Workflow

AI programming assistants are recommended to use the following tool chain to complete the "modify code → check impact → commit" loop:

1. `codelattice_project_overview` — Quickly understand project scale
2. `codelattice_changed_symbols` — Auto-detect which symbols are affected by current git diff
3. `codelattice_impact_preview` — Evaluate impact scope for each changed symbol, returning `riskReasons` (human-readable risk reasons), `impactMetrics` (quantitative metrics), `confidenceSummary` (confidence statistics), `reviewFocus` (priority review callers/files/low-confidence edges)
4. `codelattice_production_assist` — One-stop summary: quality gates + unresolved calls + change impact + `overallRisk` + `reviewChecklist` (actionable recommendations)

`codelattice_production_assist` automatically calls git diff to detect changed symbols when no `changedSymbols` parameter is provided, returning `autoDetectedChangedSymbols: true`. The `reviewChecklist` provides AI-executable recommendations: check direct callers, review low-confidence edges, run related tests, review unknown hunks.

## Rust Support Scope

Supported:

- Cargo package / workspace / target identification
- Source file ownership identification
- Symbol extraction for functions, methods, structs, enums, traits, impls, consts, statics, macro definitions, enum variants
- `use` import resolution
- `crate::`, `self::`, `super::` path resolution
- Partial same-file, same-module, and cross-file same-crate call resolution
- Enum constructor / enum variant constructor resolution
- Conservative associated function resolution
- Limited receiver type method call heuristics
- Common std/core/alloc external symbol completion
- Graph endpoint integrity quality gates

Representative Rust call resolution forms currently supported:

| Call Form | Example | Confidence |
|-----------|---------|------------|
| Same-module function | `helper()` | 0.90 |
| Import binding | `use crate::math::add; add()` | 0.85 |
| `crate::` path | `crate::math::add()` | 0.90 |
| `self::` path | `self::inner_helper()` | 0.80 |
| `super::` path | `super::parent_fn()` | 0.80 |
| Associated function | `Config::new()` | 0.75 |
| Enum constructor | `Some(42)`, `Ok(value)`, `Err(error)` | 0.80 |
| Enum variant constructor | `Event::Click(x)` | 0.80 |
| Cross-file same-crate function | `split_last_segment()` | 0.80 |
| Wildcard import disambiguation | `helper_func()` introduced via `use calculations::*` | 0.80 |
| Limited receiver method | `v.push(1)` where `let v: Vec<i32>` | 0.65 |

Explicitly not supported:

- Full type inference
- Trait solving
- Proc-macro / build.rs execution
- Macro expansion
- Full cfg evaluator
- Arbitrary third-party crate API deep resolution

## Cangjie / 仓颉 Support Scope

Supported:

- `cjpm.toml` package / workspace scanning
- Source file collection
- Symbol extraction for Function / Class / Struct / Enum / Interface / TypeAlias / Macro / Init
- Named import / alias import / wildcard import / path dependency resolution
- Same-file and cross-file reference extraction
- Function call reference extraction
- `cjc` / `cjlint` diagnostics runner integration
- Graph output
- `cangjie inspect` / `cangjie graph`
- `--strict` quality gates

Enable Cangjie feature:

```bash
cargo build --features tree-sitter-cangjie -p gitnexus-rust-core-cli --bins
```

Explicitly not supported:

- Full method dispatch
- Type inference
- Trait / interface solving
- Macro expansion
- Full cfg evaluator

## Cache and Performance

CodeLattice provides a two-layer analysis cache to accelerate repeated MCP calls:

1. **Memory Layer** (enabled by default) — In-process LRU cache with up to 16 entries. Repeated calls within the same process hit directly without re-analysis.
2. **Persistent Layer** (opt-in) — Cross-process disk cache. Enable by setting the `CODELATTICE_CACHE_DIR` environment variable.

Cache lookup order: memory → persistent → re-analysis.

### Persistent Cache Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `CODELATTICE_CACHE_DIR` | Persistent cache directory path. Enables persistent cache when set. When not set, only memory cache is used. |
| `CODELATTICE_CACHE` | Set to `off` to completely disable cache (including memory layer). |

Cache file format: `${CODELATTICE_CACHE_DIR}/cl-cache-{fnv_hash}.json`, containing analysis results, file mtime fingerprints, and manifest hash.

### Invalidation Detection

Cache automatically invalidates when the following conditions change:

- Source file added / deleted / modified (mtime detection for `.rs`/`.cj`/`.ets`/`.ts`/`.tsx`/`.js`/`.json`/`.toml`/`.md`, etc.)
- Build configuration changes (Cargo.toml / Cargo.lock / cjpm.toml / oh-package.json5 / tsconfig.json / package.json)
- CodeLattice version upgrade
- Cache file corruption

When invalidated, structured `staleReasons` are returned (e.g., `file_modified`, `manifest_changed`, `version_changed`) for AI-side cache behavior understanding.

## Output Content

`analyze --format json` outputs unified analysis results, mainly containing:

- Project summary
- Quality gate results
- Language information
- Graph nodes and edges
- Diagnostics
- Stats

Common nodes in graph data:

- Repository
- Package
- Target
- SourceFile
- Symbol
- Diagnostic

Common relationships in graph data:

- CONTAINS_PACKAGE
- HAS_TARGET
- OWNS_SOURCE
- DEFINES
- CALLS
- ACCESSES
- DESIGNATION
- HAS_PARENT
- ANNOTATES

## Known Limitations

- CodeLattice is not a compiler, IDE, or language server — it does not perform type inference, trait solving, or macro expansion
- Call edges are heuristic with confidence and reason annotations, not compiler-verified
- **TypeScript**: No path alias resolution, no monorepo/workspace support, no TSX framework hints
- **ArkTS**: Struct keyword parsed as ERROR node (recovered through pattern matching), no @Builder/@Extend
- Does not execute user project scripts
- No per-symbol incremental recompute (currently project-level full re-analysis)

## Safety Model

- CodeLattice runs locally by default and does not upload project code.
- MCP sidecar is read-only by default; `rename_preview` only previews without writing files.
- `export_bridge` only writes to `/tmp`.
- `install-mcp.sh --print-config` only prints configuration templates without modifying Codex / opencode / Claude configurations.
- `fresh-clone-smoke.sh` uses `/tmp` temporary directory by default and cleans up after completion.

## Project Status and Roadmap

**External Beta (`v0.13.0-beta.1`)** — Local production trial passed, not GA.

Currently reliable:

- Rust / Cangjie CLI analysis (Stable)
- ArkTS CLI analysis (Production Trial)
- TypeScript CLI analysis (Phase A)
- MCP sidecar 22 tools
- Two-layer persistent cache
- Stable runtime promote
- Release tarball packaging + release smoke
- Fresh clone smoke
- Local AI client integration templates

Being improved:

- Multi-platform release packages (Linux, Windows)
- Linux / openEuler native smoke certification
- Automated release CI
- TypeScript path alias / monorepo support
- TSX framework hints
- Deeper per-symbol incremental recompute

Long-term direction:

- Become an embeddable, verifiable, extensible multi-language code intelligence core
- Provide infrastructure for local code understanding, impact analysis, refactoring assistance, and AI agent workflows

## Documentation

- [CHANGELOG](CHANGELOG.md) — Version change log
- [MCP Contract](docs/architecture/mcp-v0-contract.md) — MCP tool input/output contract
- [Unified Output Contract](docs/architecture/unified-output-contract.md) — CLI output format
- [Release Versioning](docs/release-versioning.md) — Version rules
- [Install Guide](docs/release-install.md) — Tarball installation
- [Linux / openEuler Source Build](docs/platforms/linux-openeuler.md) — Source-build compatibility guide
- [Upgrade Guide](docs/release/upgrade.md) — Upgrade/rollback/cache cleanup
- [Smoke Matrix](docs/release/smoke-matrix.md) — Verification matrix
- [Getting Started](docs/getting-started.md) — Detailed getting started guide

## Development and Verification

Build:

```bash
./scripts/build.sh
```

Quick smoke:

```bash
./scripts/smoke.sh --quick
```

Full local verification:

```bash
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
bash scripts/install-mcp.sh --doctor
bash scripts/codelattice-mcp.sh --self-test
bash scripts/package-release.sh
bash scripts/release-smoke.sh
bash scripts/fresh-clone-smoke.sh --skip-tests
```

More complete MCP verification:

```bash
bash scripts/mcp-dogfood.sh
bash scripts/mcp-real-client-dry-run.sh
bash scripts/mcp-local-client-smoke.sh
```

## Project Structure

```text
codelattice/
  Cargo.toml
  crates/
    project-model/       Rust project model, symbols, imports, calls, graph output
    cangjie/             Cangjie project model, symbols, diagnostics, graph output
    cli/                 Command-line entry, unified output, MCP server, language detection
  fixtures/
    call-resolution/     Rust call resolution fixture
    import-use/          Rust import fixture
    item-extraction/     Rust symbol extraction fixture
    rust/                Rust graph contract fixture
    cangjie/             Cangjie fixture
  docs/
    architecture/        Architecture and output format documentation
    decisions/           Design decisions
    fixtures/            Fixture index
    plans/               Preflight / execution / closure documentation
  scripts/
    build.sh
    smoke.sh
    codelattice-mcp.sh
    install-mcp.sh
    promote-to-local-tool.sh
    package-release.sh
    release-smoke.sh
    fresh-clone-smoke.sh
```

## License

MIT License. See [LICENSE](LICENSE).
