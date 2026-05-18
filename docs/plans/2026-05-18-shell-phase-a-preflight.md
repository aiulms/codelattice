# Shell Phase A Preflight

Date: 2026-05-18

## Goal

Add a conservative Shell script analysis path to CodeLattice so mixed automation repositories and script-heavy projects can produce useful local graph context without executing any shell code.

## Scope

- New `gitnexus-shell` crate with static line-based extraction.
- CLI `--language shell` support for `analyze`, `quality`, and `summary`.
- MCP language schema integration for existing tools.
- Portable Shell fixture and regression tests.
- WebUI snapshot matrix and runner language list update.
- README, changelog, and plan documentation update.

## Stop-Lines

- No shell execution.
- No `shellcheck` dependency.
- No Bash/Zsh interpreter integration.
- No runtime path expansion or conditional-flow proof.
- No claims that risk diagnostics prove exploitability or safety.
- No changes outside CodeLattice repository.

## Design

Shell Phase A uses a parser-light scanner. It recognizes script files by extension or shebang, extracts function definitions, `source`/`.` includes, command invocations, environment variable reads/writes, and a bounded set of risky patterns such as recursive deletion and `curl | sh`.

Graph edges remain conservative:

- Project function name matches become `CALLS` edges with medium confidence.
- External command invocations become command nodes with lower confidence.
- `source` edges only target files that exist in the scanned project.
- Risky shell patterns are diagnostics, not proof of runtime behavior.

## Acceptance Criteria

- Shell fixture analysis returns nonzero files, symbols, calls, and diagnostics.
- MCP `tools/list` language schemas include `shell`.
- `codelattice_symbol_search` can find Shell functions.
- WebUI snapshot smoke includes Shell in the matrix.
- Full verification passes before commit.
