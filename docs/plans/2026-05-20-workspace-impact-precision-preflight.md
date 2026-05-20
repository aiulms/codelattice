# Workspace Impact Precision Pack Preflight

Date: 2026-05-20

## Goal

Make CodeLattice-native `detect-changes` useful for daily governance in large mixed workspaces by reducing low-signal workspace graph noise while preserving explicit config/script/project dependency risk.

## Problem

The current workspace-aware change review correctly finds cross-project reachability, but large repos such as CodeLattice contain many fixtures, smoke projects, and test corpora. Weak `adjacent_to` and fixture-only relationships can inflate `affectedProjects`, `recommendedFollowups`, and `crossProjectRisk`, making daily review too noisy.

## Execution Card

| Item | Decision |
|---|---|
| Write set | `crates/cli/src/lib.rs`, `scripts/codelattice-detect-changes-smoke.sh`, `CHANGELOG.md`, this preflight doc |
| Forbidden set | GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool, AI client configs, live repos |
| Default policy | Daily precision: keep direct owners and high-confidence non-fixture impacts; summarize fixture/test/adjacency-only noise |
| Escape hatch | `--include-fixtures` exposes fixture/test/demo impacts; `--strict-workspace` keeps all workspace graph impacts |
| Stop-line | Static analysis remains heuristic; no runtime/build execution; do not delete or rewrite workspace graph semantics |

## Design

Add a small precision layer after workspace graph impact collection:

- classify affected project surfaces: production, fixture, test, docs, webui, script, config, unsupported, unknown
- group affected projects into direct, high-confidence, low-confidence, fixture-only, unsupported-boundary
- default output keeps direct and high-confidence production impacts in `affectedProjects`
- default output moves fixture/test/demo/adjacency-only impacts to `suppressedProjects`
- add `workspaceImpactSummary` with policy, raw/reported/suppressed counts, and group counts
- rewrite followups to be grouped and capped rather than dumping every downstream label
- compute `crossProjectRisk` from reported high-signal impacts, config/script changes, unknown owners, and unsupported boundaries

## Verification

- Red/green smoke for fixture suppression and `--include-fixtures`
- Existing workspace config change smoke must still show affected projects and `config_refs`
- `cargo fmt --check`
- `git diff --check`
- `cargo test -p gitnexus-rust-core-cli --test productization_commands`
- `cargo test --test mcp_server`
- `scripts/codelattice-detect-changes-smoke.sh`
- `scripts/codelattice-precommit-check.sh`
