# Progress Log: P0 Cache Optimization Closure

## Session: 2026-08-02

### Current Status

- **Phase:** 8 — commit and push
- **Status:** complete

### Actions Taken

- Revalidated the prior review findings against the current worktree.
- Selected a compatibility-preserving direction: Rust trace present, non-Rust trace omitted.
- Created allowed-scope persistent planning files under `docs/plans/`.
- Completed CodeLattice impact review: all four production targets are MEDIUM risk.
- Read Rust deep-support policy and confirmed the slice does not cross semantic stop-lines.
- Created and froze the independent Phase C execution card.
- Added contract tests for Rust trace presence and non-Rust trace omission.
- Confirmed RED: the Shell full-profile assertion failed because the output still contained `analysisTrace: null`.
- Applied the minimal serialization fix: omit `None` in full output and conditionally insert the field in compact output.
- Confirmed GREEN: both Rust presence and Shell omission tests pass for full and compact profiles.
- Confirmed the pre-refactor semantic baseline: 84 IMPORTS/CALLS/graph contract tests pass.
- Recorded a five-run release baseline before the scheduling cleanup. Median trace values were `25/6/11ms` (total/import/call) for `crates/project-model` and `214/21/35ms` for the repository root.
- Replaced only the two per-file Rayon `flat_map` calls with `flat_map_iter`; the sequential package dependency `flat_map` remains unchanged.
- Rejected and reverted the `flat_map_iter` experiment. Post-change five-run medians were `31/7/12ms` and `286/28/47ms`; concurrent symbol-extraction slowdown shows environmental noise, but the candidate still had no demonstrated benefit and failed the no-regression gate.
- Closed Phase A as an evidence-driven no-op, moved Phase C implementation governance to its independent card, and aligned the AI pack, CLI contracts, consumer contract, and CHANGELOG.
- Passed `cargo fmt --check`, `git diff --check`, 106 targeted regressions, and the full `cargo test` workspace run.
- Confirmed three identical normalized analysis hashes: `69ad79e63af164bdd29d63472464025acaf8d15a74217fcebf178cdf6bdd4bf3`.
- Rebuilt the reverted release implementation and measured five-run medians of `27/6/12ms` and `228/21/38ms`, confirming recovery from the rejected candidate.
- Completed native precommit: productization, 338 MCP tests, concurrency, and 17/17 detect smoke passed; workspace risk remains critical because the mixed worktree spans 18 tracked + 11 untracked files, 42 unknown hunks, 16 affected projects, and 2 unsupported boundaries.
- User authorized scoped staging, commit, and push after reviewing the critical mixed-worktree warning; submission audit started.
- Confirmed branch `master` and selected `gitcode/master` as the only push target.
- Classified the AI tool-surface changes as a second coherent commit and excluded `.cursor/`, `.omo/`, and the credential-bearing secondary remote from the publication workflow.
- Audited all tracked and untracked paths, froze three exact commit sets, and added a Phase C publication addendum reflecting the user's post-warning authorization.
- Final native detect-changes still reports workspace `critical` / symbol aggregate `HIGH` because of 18 tracked + 11 untracked files and 42 unknown hunks; the five mapped production symbols in the compact report are individually LOW. Authorization after warning remains in force.
- Staged commit A with exactly 12 AI tool-surface paths; staged diff is 320 insertions / 34 deletions and `git diff --cached --check` passes.
- Commit A created as `0e207a8` (`fix(mcp): align AI tool surface contracts`).
- Commit B first staged check found only two Markdown whitespace issues in new execution cards; both were corrected before commit.
- Commit B created as `598069d` (`perf(project-model): parallelize import and call resolution`) after the corrected staged check passed.
- Pushed commits `0e207a8` and `598069d` to `gitcode/master`; remote hooks passed and the remote advanced from `4e2dca7` to `598069d`.
- Prepared this three-file closure record as the final documentation commit; excluded `.cursor/` and `.omo/` paths remain untouched.

### Test Results

| Test | Expected | Actual | Status |
|---|---|---|---|
| Pre-closure full test suite | Pass | Passed in review turn | baseline |
| Pre-closure deterministic local analysis | Equal after volatile fields removed | Three SHA-256 hashes equal | baseline |
| `analyze_shell_full_and_compact_omit_analysis_trace` (RED) | Fail on legacy null field | Failed on full profile for the expected reason | RED confirmed |
| Productization `analysis_trace` tests (GREEN) | 2 pass | 2 passed | PASS |
| Pre-refactor IMPORTS/CALLS/graph regression bundle | 84 pass | 84 passed | baseline |
| `flat_map_iter` release experiment | No regression with measurable benefit | No benefit; raw medians regressed | REVERTED |
| Final targeted regression bundle | 106 pass | 106 passed | PASS |
| Full workspace `cargo test` | Pass | 0 failed | PASS |
| Normalized determinism | Three identical hashes | Three identical hashes | PASS |
| Native precommit | All component gates pass | PASS; workspace review risk critical | PASS with risk warning |

### Errors

| Error | Resolution |
|---|---|
| Default planning directory was outside the repository write set | Removed it and used `docs/plans/` equivalents. |
