# CodeLattice Native Detect-Changes Closure

Date: 2026-05-19

## Outcome

CodeLattice now has a first-party `codelattice detect-changes` CLI command for local pre-commit change review. It wraps the existing CodeLattice MCP analysis path inside the current binary and emits a stable JSON envelope for automation and human review.

## Delivered

- Added `detect-changes` CLI subcommand.
- Reused `codelattice_changed_symbols` and `codelattice_production_assist` via the current binary's MCP stdio mode.
- Added `codelattice.detectChanges.v1` output envelope.
- Added untracked file reporting for `--scope all` / `head` through `git ls-files --others --exclude-standard`.
- Added productization regression tests for git and non-git paths.
- Added `scripts/codelattice-detect-changes-smoke.sh`.
- Documented usage in README and CHANGELOG.

## Output Notes

- `affectedProcessCount` is intentionally `null`.
- `affectedProcessModel` is `"notAvailable"` because CodeLattice does not expose the legacy GitNexus process model.
- `generatedFrom.staticAnalysis=true`, `runtimeVerified=false`, `coverageVerified=false`, and `nativeCodeLattice=true`.
- `--scope all` maps to `git diff HEAD` for tracked files and additionally reports untracked files.

## Verification

- `cargo fmt --check` — pass
- `git diff --check` — pass
- `cargo test --test productization_commands detect_changes -- --nocapture` — pass
- `scripts/codelattice-detect-changes-smoke.sh` — pass, 9/9
- `cargo test --test mcp_server` — pass, 120/120
- `scripts/codelattice-mcp.sh --self-test` — pass, 50 tools
- `scripts/install-mcp.sh --doctor` — pass, 8/8
- `scripts/mcp-dogfood.sh` — pass, 48/48

Full `cargo test` and final external governance checks are run after this closure document is added.

## Boundary Review

- GitNexus-RC was not modified.
- GitNexus-RC-Tool was not modified.
- CodeLattice-Tool stable runtime was not promoted or modified.
- AI client configs were not modified.
- Real project source repos were not modified.
- Private untracked directories remain uncommitted.

## Follow-Up

- Update internal governance text to prefer `codelattice detect-changes` for CodeLattice-native checks once downstream workflows have consumed the new JSON envelope.
- Consider adding a WebUI pre-commit card that reads this command output.
