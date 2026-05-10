# Run #003 Format Hygiene Cleanup — Closure Review

> **日期：** 2026-05-10
> **类型：** Closure Review
> **状态：** ✅ PASS — Format drift resolved, repo hygiene restored
> **Base commit：** `7bb49cc`
> **关联：** [Run #003 Log](2026-05-10-periodic-alpha-trial-run-003.md)、[Evidence Board](2026-05-10-beta-readiness-evidence-board.md)、[Go/No-Go #003](2026-05-10-beta-readiness-go-no-go-review-003.md)

---

## 1. Background

External AI Run #003 (commit `ad795a6`) executed successfully: Rust/Cangjie real-project bridge trials both PASS, Tool ingestion SUCCESS, stdout purity PASS. However, Run #003 is recorded as **FAIL / not counted** because the mandatory baseline check `cargo fmt --check` failed on two tracked test files.

Run #003 的 FAIL 结论**不可覆盖**。本轮任务仅为后续 External AI Run #004 清除 format hygiene blocker。

---

## 2. Root Cause

Commit `ad795a6` added 6 new test functions across two files, written by an AI session without applying `cargo fmt`. The test code contained:

- Method chains exceeding rustfmt line width (e.g., `calls.iter().find(|c| c["rawText"].as_str() == Some("Vec::new()")).expect(...)`)
- Multi-line `assert!` / `assert_eq!` calls not following rustfmt conventions
- `cargo fmt` was not run before commit

Affected files:
1. `crates/cangjie/tests/constructor_extraction.rs` — 2 new test functions
2. `crates/cli/tests/project_model_call_expected_compare.rs` — 4 new test functions

---

## 3. Fix Applied

```bash
cargo fmt
```

**Only formatting changes.** No logic, behavior, or runtime changes. Verified by:
- `git diff --check` — clean (no whitespace errors)
- `git diff --stat` — only the 2 affected files
- Manual diff review — all changes are line wrapping, indentation, method-chain formatting

---

## 4. Verification Results

| Check | Result |
|-------|--------|
| `cargo fmt --check` | ✅ PASS (0 diffs) |
| `git diff --check` | ✅ PASS |
| `cargo test --test project_model_call_expected_compare` | ✅ 11/11 passed |
| `cargo test --features tree-sitter-cangjie --test constructor_extraction` | ✅ 14/14 passed |
| `cargo test --test bridge_roundtrip` | ✅ 13/13 passed |
| `cargo test --features tree-sitter-cangjie --test bridge_roundtrip` | ✅ 26/26 passed |
| `alpha-trial-smoke.sh --rust-only` | ✅ PASS: 5, FAIL: 0, SKIP: 1 |
| `alpha-trial-smoke.sh --cangjie-only` | ✅ PASS: 5, FAIL: 0, SKIP: 1 |
| `cargo test` (full suite) | ✅ All passed |
| `cargo test --features tree-sitter-cangjie` (full suite) | ✅ All passed |
| Tool `status` | ✅ up-to-date (7bb49cc) |
| Tool `detect-changes --repo codelattice` | ✅ 5 symbol changes (test functions from prior commit), risk: low |
| Tool `detect-changes --repo gitnexus-rc` | ✅ No changes detected |

---

## 5. What Was NOT Changed

- ❌ No runtime code changes
- ❌ No graph output changes
- ❌ No language behavior changes
- ❌ No GitNexus-RC runtime/schema/WebUI changes
- ❌ No GitNexus-RC-Tool changes
- ❌ No live repo (cangjie, open-nwe) modifications
- ❌ No Run #003 conclusion change — Run #003 remains FAIL / not counted
- ❌ No Run #004 execution — this round only clears the blocker

---

## 6. Run #003 Status (Unchanged)

Run #003 保持 **FAIL / not counted**。原因：外部 AI 在 baseline 阶段检测到 `cargo fmt --check` 失败，按 Runbook 规则判定为 blocking failure。Bridge trial 子项成功不能覆盖 baseline failure。

---

## 7. Next Steps

- **External AI Run #004**：应由外部 AI 在干净 repo 上独立执行，使用更新后的 Task Package
- Run #004 应参考 Run #003 的 Task Package，但 commit 应基于本轮 format cleanup 之后的 HEAD
- Beta evidence 仍需 ≥ 5 beta-countable PASS runs + 外部 AI 独立 PASS + ≥ 3 周日历跨度
