# CodeLattice Native Governance Migration Preflight

Date: 2026-05-20

## Goal

Move CodeLattice daily self-review from legacy GitNexus-Tool `detect-changes` to first-party CodeLattice commands and scripts.

## Scope

- Add a native precommit bundle script.
- Update local governance docs so CodeLattice-native checks are the default.
- Keep GitNexus-Tool as fallback/comparison only.
- Do not remove historical GitNexus-RC migration references.
- Do not modify GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool, AI client configs, or real project repos.

## Design

The default precommit path becomes:

```bash
scripts/codelattice-precommit-check.sh
```

The script runs focused checks:

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands`
- `cargo test --test mcp_server`
- `scripts/codelattice-detect-changes-smoke.sh`
- `codelattice detect-changes --scope all --compact`

Full `cargo test` remains available through `--full`.

## Risk Notes

- This is workflow/documentation migration plus a shell script; it does not change analyzer semantics.
- The script can report `high` or `critical` risk without failing by default, because static risk requires human review rather than automatic rejection.
- `--fail-on-high-risk` is available for stricter local gates.

## Verification Plan

- `scripts/codelattice-precommit-check.sh`
- `scripts/codelattice-precommit-check.sh --help`
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands`
- `cargo test --test mcp_server`
- `scripts/codelattice-detect-changes-smoke.sh`
- Native `codelattice detect-changes` on this repo
