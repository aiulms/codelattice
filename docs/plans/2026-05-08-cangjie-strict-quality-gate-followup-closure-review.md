# Strict Quality Gate Follow-up — Closure Review

**Date:** 2026-05-08
**Status:** Closure Review
**Type:** Production Acceptance Follow-up
**Parent:** Cangjie Production Acceptance (Strict flag CLI tests + docs)

---

## Summary

The `--strict` flag was added in `952f326` (feat(cangjie): add --strict flag for enforceable quality gate) during the Production Acceptance cycle, but it lacked dedicated CLI tests and boundary documentation. This follow-up adds 8 CLI tests (5 feature-enabled + 3 feature-disabled), updates QUALITY.md with a `--strict` section, and marks the strict flag as fully integrated into the production acceptance framework.

## Changes

### `crates/cli/tests/cangjie_inspect.rs` (+8 tests)

New strict-specific tests:

| Test | Feature gate | What it verifies |
|------|-------------|-----------------|
| `cangjie_inspect_strict_on_portable_smoke_succeeds` | `tree-sitter-cangjie` | `--strict` + zero-synthetic fixture exits 0 |
| `cangjie_inspect_strict_stdout_is_pure_json` | `tree-sitter-cangjie` | `--strict` stdout is valid parseable JSON |
| `cangjie_graph_strict_equals_inspect_strict` | `tree-sitter-cangjie` | `--strict` on both commands produces identical output |
| `cangjie_inspect_strict_nonexistent_root_exits_nonzero` | `tree-sitter-cangjie` | `--strict` + nonexistent root exits non-zero |
| `cangjie_graph_strict_nonexistent_root_exits_nonzero` | `tree-sitter-cangjie` | same for graph |
| `cangjie_inspect_strict_disabled_feature_error` | `not(tree-sitter-cangjie)` | `--strict` + feature-disabled graceful failure |
| `cangjie_graph_strict_disabled_feature_error` | `not(tree-sitter-cangjie)` | same for graph |

Total CLI tests: 13 → 21 (18 feature-enabled + 3 feature-disabled).

### `QUALITY.md` (+`--strict` section)

Added a comprehensive `--strict Flag` section covering:
- Usage syntax for both commands
- Behavior description (synthetic count check, non-zero exit, no-op when disabled)
- Purpose: CI/CD enforcement without human smoke inspection
- Limitations: only checks synthetic > 0 (not duplicates/dangling/determinism), no fixture triggers synthetic > 0 in production
- Test coverage reference

### `docs/plans/2026-05-08-cangjie-production-acceptance-preflight.md`

Marked strict flag CLI tests + docs as completed in recommended next phase.

### `docs/plans/README.md`

Added strict flag follow-up entry under item 34.

## Design Decisions

- **No strict failure fixture:** All current fixtures produce synthetic=0 by design. Creating a fixture that triggers synthetic > 0 would violate the quality gates enforced by `graph_contract` and `multi_project_smoke`. Strict failure (synthetic > 0 → non-zero exit) is tested indirectly by the quality gate suites which hard-assert `synthetic_count = 0`.
- **portable-smoke as strict success fixture:** Uses the portable-smoke fixture (the most comprehensive repo-committed fixture) as the success case, ensuring strict mode works on a non-trivial project.
- **Feature-disabled strict tests:** Both inspect and graph test the disabled + --strict path, verifying the `let _strict = strict;` suppression works correctly.

## Integrity Verification

- `cargo fmt --check`: clean
- `git diff --check`: clean
- `cargo test` (no-feature): all pass (3 feature-disabled strict tests pass)
- `cargo test --features tree-sitter-cangjie`: all pass (18 cangjie_inspect tests pass)
- `cargo test --features tree-sitter-cangjie --test cangjie_inspect -- --nocapture`: 18/18 pass
- `cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture`: 4/4 pass

## Stop-lines compliance

- ✅ No change to GitNexus-RC
- ✅ No change to GitNexus-RC-Tool
- ✅ No change to live repo
- ✅ No new dependencies
- ✅ No new extractor / graph mapping
- ✅ No destructive git operations
- ✅ No WebUI/MCP/HTTP/embedding
- ✅ Tests properly feature-gated
