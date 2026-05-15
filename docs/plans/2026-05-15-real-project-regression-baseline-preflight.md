# Real Project Regression Baseline Preflight

Date: 2026-05-15

## Goal

Upgrade the GitCode real-project corpus smoke from "runs and returns non-empty
graphs" into a regression gate that can compare current metrics against a saved
baseline. This is the second block of the Multi-Language Production Hardening
Pack.

The target is release safety, not deeper language semantics. The runner should
catch sharp drops in graph availability, large runtime regressions, and missing
language-feature builds before a release is considered healthy.

## Scope

Write set:

- `scripts/real-project-corpus-smoke.py`
- `scripts/real-project-corpus-smoke-test.py`
- `docs/real-project-corpus-baseline.json`
- `docs/real-project-corpus.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-15-real-project-regression-baseline-closure.md`
- `CHANGELOG.md`

Forbidden set:

- No vendored third-party repositories.
- No target project source modification.
- No target build, package-manager, or test commands.
- No GitNexus-RC runtime/schema/WebUI changes.
- No GitNexus-RC-Tool changes.
- No AI client config changes.
- No changes to language adapters unless the regression gate exposes a blocker.

## Design

Add a baseline JSON file with per-target metrics and budget policy. Extend the
smoke runner with three baseline modes:

- default: keep existing smoke behavior unchanged;
- `--compare-baseline`: compare current results with the baseline and fail when
  a target exceeds configured regression budgets;
- `--accept-baseline`: write a new baseline from the current successful results
  after intentional changes.

The baseline comparison should classify results:

- `pass`: target passed minimum thresholds and stayed within budget;
- `warn`: metric changed beyond a warning threshold but below failure threshold;
- `fail`: target failed minimum thresholds or crossed a failure budget;
- `missing-baseline`: target has no baseline entry, which is a warning unless
  strict comparison is requested.

Metrics tracked in Phase 1:

- `nodeCount`
- `edgeCount`
- `symbolCount`
- `sourceFileCount`
- `elapsedSeconds`

Budgets:

- count metrics use percentage-drop budgets;
- elapsed time uses percentage-increase budgets;
- budgets are intentionally loose because GitCode targets can drift upstream.

## Risk

| Risk | Mitigation |
|------|------------|
| Upstream target changes make exact counts noisy | Use percentage windows, not exact equality. |
| First baseline becomes stale quickly | `--accept-baseline` makes intentional updates explicit and reviewable. |
| Runtime varies by local machine | elapsedSeconds warns by default and only fails on large increases. |
| Baseline comparison masks normal smoke failures | Minimum threshold failures remain hard failures before budget comparison. |
| Script complexity grows unchecked | Add standard-library tests for compare/accept behavior. |

## Verification Plan

- Red/green tests for baseline compare and accept behavior.
- `python3 -m py_compile scripts/real-project-corpus-smoke.py scripts/real-project-corpus-smoke-test.py`
- `python3 scripts/real-project-corpus-smoke-test.py`
- `python3 scripts/real-project-corpus-smoke.py --offline --compare-baseline --cache-dir /tmp/codelattice-real-corpus-smoke`
- `python3 scripts/real-project-corpus-smoke.py --offline --compare-baseline --json-out /tmp/codelattice-real-corpus-results.json`
- `cargo fmt --check`
- `git diff --check`
- `cargo test`
- GitNexus `detect-changes`
