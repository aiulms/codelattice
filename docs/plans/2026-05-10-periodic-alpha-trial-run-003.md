# Periodic Alpha Trial Run #003

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** 外部 AI 独立执行 trial 记录（实际执行，非模板）
> **执行者：** external AI independent run (Codex)
> **关联：** [External AI Task Package](2026-05-10-external-ai-periodic-alpha-trial-run-003-task-package.md)、[Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)、[Evidence Board](2026-05-10-beta-readiness-evidence-board.md)

---

## Independent Truth Gate

| Repo | HEAD / branch | Remote | Initial state |
|------|---------------|--------|---------------|
| CodeLattice | `ad795a6029ce309383ff2e1e8b405a2560d47d02` / `master` | `gitcode=https://gitcode.com/aiulms/codelattice.git` | clean; Tool index initially stale, refreshed to `codelattice` |
| GitNexus-RC | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` / `main` | `origin/gitcode=https://gitcode.com/aiulms/gitnexus-rc.git` | existing untracked files only; not modified |
| GitNexus-RC-Tool | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` / `main` | `origin=https://gitcode.com/aiulms/gitnexus-rc.git` | clean; read-only use |
| cangjie-GitNexus-Index | `9b29db6b7599547205e15034489e7c4e13f879f3` / detached HEAD | `origin=https://gitcode.com/aiulms/cjgui.git` | clean; header artifacts restored after Tool import |

Initial Tool registry:
- `codelattice` present.
- Initial `status` for CodeLattice was stale (`indexed commit 1b74e3f`, current `ad795a6`).
- Refreshed with:
  ```
  node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
  ```
- Refresh result: `4,330 nodes | 7,510 edges | 120 clusters | 157 flows`.
- `detect-changes --repo codelattice --scope all`: No changes detected.

---

## Baseline Smoke

| Check | Result |
|-------|--------|
| `git diff --check` | PASS |
| `cargo fmt --check` | **FAIL** |
| `bash -n scripts/build.sh scripts/smoke.sh scripts/alpha-trial-smoke.sh` | PASS |
| `scripts/alpha-trial-smoke.sh --rust-only` | PASS: 5, FAIL: 0, SKIP: 1 |
| `scripts/alpha-trial-smoke.sh --cangjie-only` | PASS: 5, FAIL: 0, SKIP: 1 |

`cargo fmt --check` failed before any source edit. It reported formatting drift in tracked test files:
- `crates/cangjie/tests/constructor_extraction.rs`
- `crates/cli/tests/project_model_call_expected_compare.rs`

No runtime/source formatting changes were applied because Run #003 is docs-only validation and the task forbids runtime changes. This mandatory baseline failure makes Run #003 **not beta-countable**, even though the bridge trials below completed.

Smoke cleanup checks after each smoke:
- Tool registry still contained `codelattice`.
- No permanent `alpha-trial-rust-smoke` or `alpha-trial-cangjie-smoke` registry entry remained.
- No CodeLattice `.claude/` or `CLAUDE.md` remained after cleanup.
- `detect-changes --repo codelattice --scope all` ran successfully.

---

## Trial #3-A — Rust self-analysis

- **Date:** 2026-05-10 10:36-10:38 CST
- **Executor:** external AI independent run (Codex)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/codelattice
- **Language:** rust
- **CodeLattice HEAD:** `ad795a6`
- **Command:**
  ```
  cargo run -- analyze --root /Users/jiangxuanyang/Desktop/codelattice --language rust --format gitnexus-rc --strict > /tmp/codelattice-run003-rust-V2dk5s
  ```

- **Bridge JSON size:** 1,948,073 bytes
- **Stdout JSON purity:** PASS
  - First byte: `{` (`0x7b`)
  - `python3 -m json.tool /tmp/codelattice-run003-rust-V2dk5s >/dev/null`: PASS
  - No sed cleanup used

- **Tool ingestion:** SUCCESS
  - Command:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-run003-rust-V2dk5s
    ```
  - Result: `5,078 nodes | 7,510 edges | 120 clusters | 157 flows`
  - Non-fatal adapter warnings: Diagnostic / stdlib symbol kinds fell back to `CodeElement`; no validation failure.

- **Bridge JSON Stats:**
  - schemaVersion: `0.3.0`
  - generatedAt: present; excluded from deterministic compare
  - Packages: 7
  - Source files: 58
  - Symbols: 1,636
  - Diagnostics: 728
  - Nodes total: 1,702
  - Edges total: 2,635
  - Edge breakdown:
    - calls: 1,140
    - defines: 852
    - accesses: 138
    - designations: 25
    - contains: 7
    - owns: 33
    - annotates: 25
    - other: 415

- **Quality checks:**
  - Dangling source/target: 0
  - Duplicate node IDs: 0
  - Duplicate edge triples: 0
  - Stats consistency: PASS
  - Deterministic: PASS (`generatedAt` excluded; second run matched exactly)
  - Quality gates (`--strict`): PASS (exit 0)

- **Tool status / detect-changes:**
  - `status`: up-to-date after Rust import.
  - `detect-changes --repo codelattice --scope all` immediately after Rust import reported header artifact changes in `AGENTS.md`.
  - Cleanup restored `AGENTS.md`, deleted untracked `.claude/` and `CLAUDE.md`, then refreshed:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
    ```
  - Post-cleanup `detect-changes --repo codelattice --scope all`: No changes detected.

- **Cleanup performed:**
  - [x] Temporary Rust bridge JSON deleted
  - [x] Deterministic comparison temp JSON deleted
  - [x] Tool-generated `.claude/` and `CLAUDE.md` removed
  - [x] Tool header change to `AGENTS.md` restored
  - [x] CodeLattice registry restored to `codelattice`

- **Failure classification:** NONE for Rust bridge trial; overall run still blocked by baseline `cargo fmt --check`.
- **Rollback action:** Not required; cleanup/re-index only.
- **Final status:** Rust bridge trial PASS, Run #003 overall FAIL / not counted.

---

## Trial #3-B — Cangjie cjgui

- **Date:** 2026-05-10 10:39-10:44 CST
- **Executor:** external AI independent run (Codex)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui
- **Language:** cangjie
- **CodeLattice HEAD:** `ad795a6`
- **Command:**
  ```
  cargo run --features tree-sitter-cangjie -- analyze --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui --language cangjie --format gitnexus-rc --strict > /tmp/codelattice-run003-cangjie-7Ow9AW
  ```

- **Bridge JSON size:** 1,031,702 bytes
- **Stdout JSON purity:** PASS
  - First byte: `{` (`0x7b`)
  - `python3 -m json.tool /tmp/codelattice-run003-cangjie-7Ow9AW >/dev/null`: PASS
  - No sed cleanup used

- **Tool ingestion:** SUCCESS after running from the target checkout
  - Initial import from CodeLattice cwd succeeded but temporarily polluted the `codelattice` registry; this exposed cwd-sensitive Tool behavior and was cleaned up.
  - Effective command from `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index`:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-run003-cangjie-7Ow9AW
    ```
  - Result: `7,219 nodes | 14,314 edges | 75 clusters | 300 flows`
  - Bridge graph loaded: `7,221 nodes, 17,568 relationships`

- **Bridge JSON Stats:**
  - schemaVersion: `v1.0.0`
  - generatedAt: present; excluded from deterministic compare
  - Packages: 1
  - Source files: 14
  - Symbols: 887
  - Diagnostics: 0
  - Nodes total: 903
  - Edges total: 3,252
  - Edge breakdown:
    - defines: 887
    - uses: 2,350
    - contains: 1
    - owns: 14

- **Quality checks:**
  - Dangling source/target: 0
  - Duplicate node IDs: 0
  - Duplicate edge triples: 0
  - Stats consistency: PASS
  - Deterministic: PASS (`generatedAt` excluded; second run matched exactly)
  - Quality gates (`--strict`): PASS (exit 0)

- **Tool status / detect-changes / context:**
  - `status`: up-to-date after target-checkout import.
  - `detect-changes --repo cjgui --scope all` immediately after bridge import reported only header artifact changes in `AGENTS.md` / `CLAUDE.md`.
  - `context main --repo cjgui`: returned ambiguous result with 2 candidates:
    - `Function:labs/cffi_smoke/src/main.cj:main`
    - `Function:labs/macos_bridge_smoke/src/main.cj:main`
  - Restored `AGENTS.md` / `CLAUDE.md`, then refreshed index checkout with:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index --force --skip-agents-md --name cjgui --allow-duplicate-name
    ```
  - Post-cleanup `detect-changes --repo cjgui --scope all`: No changes detected.

- **Cleanup performed:**
  - [x] Temporary Cangjie bridge JSON deleted
  - [x] Deterministic comparison temp JSON deleted
  - [x] cangjie-GitNexus-Index `AGENTS.md` / `CLAUDE.md` header artifacts restored
  - [x] cangjie-GitNexus-Index clean except ignored `.gitnexus/`
  - [x] CodeLattice registry restored to `codelattice`

- **Failure classification:** NONE for Cangjie bridge trial; cwd-sensitive import/header artifacts required cleanup.
- **Rollback action:** Not required; cleanup/re-index only.
- **Final status:** Cangjie bridge trial PASS, Run #003 overall FAIL / not counted.

---

## Summary

| Item | Rust (#3-A) | Cangjie (#3-B) |
|------|-------------|----------------|
| Target | CodeLattice self-analysis | cangjie-GitNexus-Index/runtime/cjgui |
| JSON size | 1,948,073 bytes | 1,031,702 bytes |
| Nodes | 1,702 | 903 |
| Edges | 2,635 | 3,252 |
| Symbols | 1,636 | 887 |
| Source files | 58 | 14 |
| Dangling | 0 | 0 |
| Duplicate node IDs | 0 | 0 |
| Duplicate edge triples | 0 | 0 |
| Deterministic | PASS, excluding generatedAt | PASS, excluding generatedAt |
| Stdout purity | PASS | PASS |
| Tool import | SUCCESS | SUCCESS |
| detect-changes after cleanup | No changes detected | No changes detected |
| Bridge trial status | PASS | PASS |

**Overall Run #003: FAIL / NOT COUNTED.**

Reason: mandatory baseline and final verification include `cargo fmt --check`, which failed on pre-existing tracked test formatting drift before any source edit. The Rust and Cangjie real-project bridge trials themselves were executable, ingestible, deterministic, duplicate-free, dangling-free, and cleanable.

---

## Failure Classification

| Classification | Result |
|----------------|--------|
| stdout purity | PASS |
| dangling endpoint | PASS |
| duplicate node/edge | PASS |
| deterministic drift | PASS |
| adapter validation | PASS |
| header artifact | RECOVERED |
| command authority | PASS |
| baseline verification | **FAIL: `cargo fmt --check`** |

**Rollback action:** Not required. Cleanup actions were sufficient:
- restored Tool-modified header files;
- deleted temporary bridge JSON files;
- restored Tool registry to `codelattice`;
- did not modify runtime code or live repos.

---

## Beta Criteria Impact

Run #003 does **not** count toward beta criteria because final status is FAIL.

| Criterion | Impact |
|-----------|--------|
| Trial count | remains 2/5 beta-countable PASS runs |
| Trial logs | this is an actual failure log, but PASS logs remain 2/3 |
| External AI independent run | attempted, but not PASS |
| Tool ingestion stability | Rust and Cangjie ingestion succeeded in this run |
| Stdout purity | PASS |
| Endpoint integrity | PASS |
| Beta | NOT YET |

---

## Constraint Confirmation

- 未切默认工具
- 未把 CodeLattice 设为默认 analyze engine
- 未修改 GitNexus-RC runtime/schema/WebUI
- 未修改 GitNexus-RC-Tool
- 未修改 `/Users/jiangxuanyang/Desktop/cangjie` live repo
- 未修改 open-nwe 或其他生产项目
- 未放宽 bridge adapter validator
- 未新增依赖
- 未重命名 Cargo package / binary
- 未重命名 `--format gitnexus-rc`
- 未重命名 `--experimental-rust-core-bridge-graph`
- 未提交 `.claude/`、`CLAUDE.md`、`.sisyphus/`、临时 bridge JSON、编译产物
- 生产 GitNexus 命令均使用 Tool CLI 绝对路径
- 未使用 `npx gitnexus` 作为生产命令
- stdout JSON purity 为 stdout-only 验证，未使用 sed
- `generatedAt` 未参与 deterministic strict compare
