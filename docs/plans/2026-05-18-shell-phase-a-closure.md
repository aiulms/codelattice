# Shell Phase A Closure

Date: 2026-05-18

## Summary

Shell Phase A adds static graph support for shell-script-heavy projects. The implementation is intentionally bounded: it extracts useful project structure for AI review workflows while preserving the CodeLattice rule that target project code is never executed.

## Delivered

- `crates/shell/` crate for static script discovery, extraction, and graph construction.
- Shell fixture at `fixtures/shell/portable-smoke/`.
- CLI integration for `analyze`, `quality`, and `summary`.
- Bridge conversion via `crates/cli/src/shell_bridge.rs`.
- MCP schema and initialize metadata include Shell support.
- MCP regression tests for language schema, analyze, and symbol search.
- WebUI runner/snapshot language lists include Shell.
- WebUI fixture snapshot matrix includes Shell.
- README and changelog describe Shell scope and limitations.

## Safety Boundaries

- Shell scripts are never executed.
- Shell diagnostics are candidates for review, not proof of exploitability.
- Dynamic shell behavior such as complex parameter expansion, runtime `source`, eval-like constructs, command substitution semantics, and conditional reachability are out of scope for Phase A.
- The implementation does not replace shellcheck, CI, or manual script review.

## Verification

Final verification is recorded in the commit report. Expected gates:

- `cargo fmt --check`
- `git diff --check`
- `cargo test -p gitnexus-shell`
- `cargo test --test mcp_server`
- `cargo test`
- WebUI snapshot/viewer smoke
- MCP self-test and dogfood
- GitNexus detect-changes
- Tool index refresh

## Follow-Up

- Shell risk review pack: richer diagnostics for destructive operations and unsafe downloads.
- Shell include/source path refinement for runtime variables.
- CI/workflow file analysis for GitHub Actions, GitCode CI, and local release scripts.
