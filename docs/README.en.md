# CodeLattice English Reference

> The Chinese [README](../README.md) is the authoritative project introduction. This English page is a reference for external beta users and downstream integrators.

CodeLattice is a Rust-native local code graph analysis core for AI coding tools, code review, and engineering quality workflows. It analyzes source code locally, extracts symbols, resolves call relationships, produces structured graph output, runs quality gates, and exposes the result through CLI and MCP sidecar tools.

Current release status: **External Beta (`v0.13.0-beta.2`)**. Rust and Cangjie are the stable language lines; ArkTS / HarmonyOS is in production trial; TypeScript is in Phase A.

## What It Does

- Builds a local project model for packages, targets, source files, and ownership.
- Extracts functions, methods, types, traits/interfaces, enums, macros, init functions, and language-specific symbols.
- Resolves same-module, cross-file, import-binding, conservative associated-function, and limited receiver-method calls.
- Emits repository / package / source file / symbol / diagnostic graph nodes and relationship edges.
- Runs graph quality gates such as dangling-edge, duplicate, statistics consistency, stdout JSON purity, and deterministic-output checks.
- Provides 22 MCP tools for project overview, symbol context, call queries, impact preview, changed-symbol detection, production assist, and cache management.

## Why Rust

Rust is part of the product boundary, not only an implementation detail:

- It keeps local scanning and graph construction fast enough for repeated AI sidecar usage.
- It supports small, predictable binaries and a clear cross-platform release path.
- Its memory-safety and explicit error handling fit read-only local analysis tools.
- It makes deterministic output, fixture-based testing, confidence/reason reporting, and release smoke gates easier to enforce.

## How CodeLattice Differs

| Compared With | Difference |
|---------------|------------|
| Hosted code-intelligence services | Runs locally by default and does not upload source code |
| IDE / LSP-only features | Exposes structured CLI / MCP output for automation, not just editor UX |
| grep / ctags-style tools | Produces project models, symbol graphs, call edges, quality reports, and impact signals |
| Full compilers or static analyzers | Focuses on practical, explainable, confidence-tagged code context rather than complete type inference |
| Generic scanners | Has deeper Rust and Cangjie project modeling, fixtures, call strategies, and quality gates |

## Quick Start

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice
bash scripts/install-mcp.sh --build
target/release/codelattice --version
```

Analyze a Rust fixture:

```bash
target/release/codelattice analyze \
  --root fixtures/rust/portable-smoke \
  --language rust \
  --format json
```

Install the stable MCP wrapper:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

Install from the current macOS Apple Silicon release package:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.13.0-beta.2 \
  --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

Linux and openEuler users should currently follow the source-build path documented in [docs/platforms/linux-openeuler.md](platforms/linux-openeuler.md).

## Supported Languages

| Language | Status |
|----------|--------|
| Rust | Stable |
| Cangjie / 仓颉 | Stable |
| ArkTS / HarmonyOS | Production Trial |
| TypeScript | Phase A |

## Safety Model

- Runs locally by default and does not upload project code.
- MCP sidecar is read-only by default.
- `rename_preview` only previews; it does not write files.
- `export_bridge` writes only to `/tmp`.
- Config commands print templates unless explicitly asked to install local tool files.

## More Documentation

- [CHANGELOG](../CHANGELOG.md)
- [MCP Contract](architecture/mcp-v0-contract.md)
- [Unified Output Contract](architecture/unified-output-contract.md)
- [Release Install Guide](release-install.md)
- [Upgrade Guide](release/upgrade.md)
- [Smoke Matrix](release/smoke-matrix.md)
