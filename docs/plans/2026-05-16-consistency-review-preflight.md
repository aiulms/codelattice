# Consistency Review (Stale Docs / Tests) — Preflight Card

**Date**: 2026-05-16 | **Branch**: master (HEAD: 7adec39) | **Target**: CodeLattice v0.24

## 1. What & Why
Code changes often leave docs and tests stale. `codelattice_consistency_review` (tool #35) cross-references changed symbols against documentation (README, API docs) and test files to flag candidates needing updates — without running tests or claiming coverage proof.

## 2. Definition
- **staleDocCandidates**: docs that mention changed/deleted/renamed symbols
- **missingDocUpdateCandidates**: public/framework-changed symbols with no related docs
- **relatedTests**: tests related to changed symbols (by path/name/graph)
- **missingTestCandidates**: changed public/framework APIs with no related tests
- **staleTestCandidates**: tests referencing unknown/dead symbols

## 3. What this is NOT
- Coverage analysis (coverageVerified=false always)
- Test runner
- Doc auto-updater
- Proof of staleness (heuristic=true always)

## 4. Write Set: CodeLattice repo only. No new deps. No runtime exec. No WebUI.
## 5. Stop-Line: baseline fails, cargo check errors > 3, forbidden file touched.
## 6. Verification: fmt → diff → tests (182→~192) → dogfood (34→35) → contract → smoke → commit/push.
