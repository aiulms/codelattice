# CodeLattice Native Governance Migration Closure

Date: 2026-05-20

## Outcome

CodeLattice daily governance now defaults to CodeLattice-native checks. Legacy GitNexus-Tool remains available only as fallback/comparison for historical process-flow review.

## Delivered

- Added `scripts/codelattice-precommit-check.sh`.
- Updated `AGENTS.md` daily rules from GitNexus-first to CodeLattice-native.
- Updated README with the native precommit bundle.
- Updated MCP local setup docs to reflect current language/workspace support and native detect-changes.
- Updated CHANGELOG and plans index.

## Native Precommit Bundle

```bash
scripts/codelattice-precommit-check.sh
```

Default checks:

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands`
- `cargo test --test mcp_server`
- `scripts/codelattice-detect-changes-smoke.sh`
- `codelattice detect-changes --root . --language rust --scope all --compact`

Optional:

- `--full` runs full `cargo test`.
- `--fail-on-high-risk` makes high/critical native risk exit non-zero.

## Boundary Review

- GitNexus-RC was not modified.
- GitNexus-RC-Tool was not modified.
- CodeLattice-Tool stable runtime was not promoted or modified.
- AI client configs were not modified.
- Real project repos were not modified.

## Verification

- `scripts/codelattice-precommit-check.sh --help` — pass
- `bash -n scripts/codelattice-precommit-check.sh` — pass
- `cargo fmt --check` — pass
- `git diff --check` — pass
- `scripts/codelattice-precommit-check.sh` — pass
  - `cargo test --test productization_commands` — 15/15
  - `cargo test --test mcp_server` — 120/120
  - `scripts/codelattice-detect-changes-smoke.sh` — 9/9
  - native `detect-changes` emitted `codelattice.detectChanges.v1`, `nativeCodeLattice=true`
- `cargo test` — pass
- `scripts/codelattice-mcp.sh --self-test` — pass, 50 tools
- `scripts/mcp-dogfood.sh` — pass, 48/48

Native detect-changes reported `high` risk for this migration because the change intentionally updates governance rules and scripts. The script warns on high/critical by default and can be made strict with `--fail-on-high-risk`.

## Follow-Up

- Consider wiring the native precommit report into the WebUI Release/Change Review panel.
- Once downstream agent prompts are refreshed, remove remaining legacy GitNexus-first wording from old historical plan files only when editing those files for other reasons.
