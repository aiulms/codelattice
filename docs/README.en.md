# CodeLattice English Reference

[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/aiulms/codelattice)

> The Chinese [README](../README.md) is the authoritative project introduction. This English page is a reference for external beta users and downstream integrators.

CodeLattice is a local code graph engine for **AI coding workflows**. It uses static analysis to turn large repositories into queryable, repeatable, auditable context: project structure, symbols, call relationships, dependency boundaries, quality signals, and change impact areas that AI assistants and developers can inspect before editing code.

It is designed for onboarding into unfamiliar repositories, maintaining legacy codebases, reviewing risky changes, and giving local AI tools such as Codex, Claude Desktop, and opencode an MCP sidecar. CodeLattice scans source code read-only by default, does not upload project code, and does not execute target project build scripts. Results are available through the CLI, JSON output, and MCP tools.

Short version: **let AI read the code map before it edits the code.**

Current release status: **External Beta / daily-use candidate (`v0.17.0-beta.1`)**. The default MCP `ai` toolset exposes 6 facade-first tools for AI agents; `full` mode keeps the 49-tool debug and regression surface. Rust and Cangjie are stable language lines; ArkTS / HarmonyOS is in production trial; TypeScript, JavaScript, C, C++, Python, and Shell are included in the hardened beta artifact.

## What It Does

- Builds a local project model for packages, targets, source files, and ownership.
- Extracts functions, methods, types, traits/interfaces, enums, macros, init functions, and language-specific symbols.
- Resolves same-module, cross-file, import-binding, conservative associated-function, and limited receiver-method calls.
- Emits repository / package / source file / symbol / diagnostic graph nodes and relationship edges.
- Runs graph quality gates such as dangling-edge, duplicate, statistics consistency, stdout JSON purity, and deterministic-output checks.
- Provides an AI-friendly 6-tool MCP default surface for workflow routing, project insight, symbol search/context, change review, workspace analysis, and cache control.
- Keeps a 49-tool `full` MCP surface for debugging, regression coverage, and lower-level graph queries.

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
  --version v0.17.0-beta.1 \
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
| TypeScript | Beta Hardened |
| JavaScript | Phase A Hardened |
| C | Phase A Hardened |
| C++ | Phase A Hardened |
| Python | Phase A Hardened |
| Shell | Phase A Hardened |

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
