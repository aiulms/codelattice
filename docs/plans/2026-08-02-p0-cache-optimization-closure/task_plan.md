# Task Plan: P0 Cache Optimization Closure

## Goal

Close the P0 cache-optimization delivery with a valid Phase C execution card, backward-compatible CLI tracing, low-risk rayon overhead cleanup, and evidence-backed verification without touching unrelated worktree changes.

## Current Phase

Phase 8 — Commit and push

## Phases

### Phase 1: Preflight and frozen scope

- [x] Revalidate review findings against current code and contracts.
- [x] Run CodeLattice impact analysis for every production symbol to be edited.
- [x] Freeze Phase C write set, forbidden set, risks, and stop-lines.
- **Status:** complete

### Phase 2: TDD contract fix

- [x] Add CLI regression tests requiring Rust `analysisTrace` and omitting it for non-Rust output.
- [x] Run the tests and record the expected RED failure.
- [x] Implement the smallest serialization/profile change that turns the tests GREEN.
- **Status:** complete

### Phase 3: Low-risk parallel cleanup

- [x] Benchmark the current release implementation on allowed local roots.
- [x] Evaluate nested rayon flattening and reject/revert `flat_map_iter` after it fails the performance gate.
- [x] Re-run deterministic fixtures and local performance samples.
- **Status:** complete

### Phase 4: Governance closure

- [x] Create a separate Phase C execution card and preserve Phase A as evidence-driven no-op closure.
- [x] Update the AI optimization pack, output contracts, and CHANGELOG consistently.
- [x] Record the strict `<3s` target as near-target unless repeatable evidence proves it met.
- **Status:** complete

### Phase 5: Closure review

- [x] Run formatting, targeted tests, full tests, deterministic comparisons, and native precommit governance.
- [x] Separate P0 conclusions from unrelated dirty-worktree risk; do not stage, commit, or push.
- [x] Deliver final status and residual risks.
- **Status:** complete

### Phase 6: Submission boundary audit

- [x] Inspect branch, remotes, tracked diffs, and untracked paths.
- [x] Classify every changed path into a coherent commit or explicit exclusion.
- [x] Freeze exact path/hunk sets; exclude editor/agent state and unrelated artifacts.
- **Status:** complete

### Phase 7: Pre-push governance

- [x] Run native change review and the required precommit gate against the final worktree.
- [x] Reconfirm the known workspace critical warning and ensure no new implementation failure exists.
- [x] Verify the staged diff before each commit.
- **Status:** complete

### Phase 8: Commit and push

- [x] Create minimal coherent commit(s) without staging excluded paths.
- [x] Push according to the repository remote/branch convention.
- [x] Record commit IDs, remote result, and residual worktree state.
- **Status:** complete

## Decisions Made

| Decision | Rationale |
|---|---|
| Keep `analysisTrace` for Rust CLI output as an optional additive field | It is useful instrumentation and already supports the evidence-driven pivot, but must not add `null` to other-language contracts. |
| Use a new Phase C card instead of rewriting the Phase A scope | Execution cards are frozen governance records; the pivot needs an explicit new boundary. |
| Treat open-nwe figures as recorded evidence only | Repository instructions prohibit production analysis of live open-nwe. |
| Reject the `flat_map_iter` follow-up | Allowed local release proxies showed no benefit and raw median regression, so evidence did not justify retaining it. |
| Stage/commit/push only after a path-and-hunk audit | The user has now authorized direct publication, but the mixed worktree still requires logical isolation. |
| Use three commits | Keep the AI tool-surface pack, P0 implementation, and closure publication record independently reviewable. |
| Temporarily omit/restore only the Performance changelog block during commit A | `CHANGELOG.md` contains both logical changes in one hunk; `apply_patch` preserves the working content while enabling exact staging without interactive ambiguity. |

## Errors Encountered

| Error | Attempt | Resolution |
|---|---:|---|
| `planning-with-files` default `.planning/` location conflicts with repository write set | 1 | Removed the newly created empty templates and placed equivalent planning files under `docs/plans/`. |
| First commit-B staged check found trailing whitespace / extra EOF blank in new execution cards | 1 | Corrected both Markdown files with `apply_patch`, then restaged and reran the check. |
