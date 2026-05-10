# Periodic Alpha Trial Run #004

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** 外部 AI 独立 retry trial 记录（实际执行，非模板）
> **执行者：** external AI independent retry after Run #003 fmt failure (Codex)
> **关联：** [Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)、[Run #003](2026-05-10-periodic-alpha-trial-run-003.md)、[Run #003 Format Hygiene Cleanup](2026-05-10-run003-format-hygiene-cleanup-closure.md)、[Evidence Board](2026-05-10-beta-readiness-evidence-board.md)

---

## Independent Truth Gate

| Repo | HEAD / branch | Remote | Initial state |
|------|---------------|--------|---------------|
| CodeLattice | `d2c519f1009a6a7e98063f8b40fd8bcaf3888b16` / `master` | `gitcode=https://gitcode.com/aiulms/codelattice.git` | clean; Tool index initially stale, refreshed to `codelattice` |
| GitNexus-RC | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` / `main` | `origin/gitcode=https://gitcode.com/aiulms/gitnexus-rc.git` | existing untracked files only; not modified |
| GitNexus-RC-Tool | `6f84a687c3b100c936ab394a7379e712a4c6d4f6` / `main` | `origin=https://gitcode.com/aiulms/gitnexus-rc.git` | clean; read-only use |
| cangjie-GitNexus-Index | `9b29db6b7599547205e15034489e7c4e13f879f3` / detached HEAD | `origin=https://gitcode.com/aiulms/cjgui.git` | clean; header artifacts restored after Tool import |

Initial Tool status for CodeLattice was stale (`indexed commit 7bb49cc`, current `d2c519f`). Refreshed with:

```
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
```

Refresh result: `4,353 nodes | 7,545 edges | 119 clusters | 157 flows`.

---

## Mandatory Gates

| Check | Result |
|-------|--------|
| `git diff --check` | PASS |
| `cargo fmt --check` | PASS |
| `cargo test --test bridge_roundtrip` | PASS — 13/13 |
| `cargo test --features tree-sitter-cangjie --test bridge_roundtrip` | PASS — 26/26 |
| `bash -n scripts/build.sh scripts/smoke.sh scripts/alpha-trial-smoke.sh` | PASS |
| `scripts/alpha-trial-smoke.sh --rust-only` | PASS — 5 PASS, 0 FAIL, 1 SKIP |
| `scripts/alpha-trial-smoke.sh --cangjie-only` | PASS — 5 PASS, 0 FAIL, 1 SKIP |

Run #003 的 `cargo fmt --check` blocker 已在 `d2c519f` 后清除，本轮 mandatory gates 全部通过。

---

## Trial #4-A — Rust self-analysis

- **Date:** 2026-05-10 12:12-12:13 CST
- **Executor:** external AI independent retry after Run #003 fmt failure (Codex)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/codelattice
- **Language:** rust
- **CodeLattice HEAD:** `d2c519f`
- **Command:**
  ```
  cargo run -- analyze --root /Users/jiangxuanyang/Desktop/codelattice --language rust --format gitnexus-rc --strict > /tmp/codelattice-run004-rust-XXXXXX.json
  ```

- **Bridge JSON size:** 1,948,073 bytes
- **Stdout JSON purity:** PASS
  - First byte: `{` (`0x7b`)
  - `python3 -m json.tool /tmp/codelattice-run004-rust-XXXXXX.json >/dev/null`: PASS
  - No sed cleanup used

- **Tool ingestion:** SUCCESS
  - Command:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-run004-rust-XXXXXX.json
    ```
  - Result: `5,102 nodes | 7,545 edges | 120 clusters | 157 flows`
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
  - `detect-changes --repo codelattice --scope all` immediately after Rust import reported only Tool header artifact changes in `AGENTS.md`.
  - Cleanup restored `AGENTS.md`, deleted untracked `.claude/` and `CLAUDE.md`, refreshed `codelattice`, then `detect-changes --repo codelattice --scope all`: No changes detected.

- **Cleanup performed:**
  - [x] Temporary Rust bridge JSON deleted
  - [x] Deterministic comparison temp JSON deleted
  - [x] Tool-generated `.claude/` and `CLAUDE.md` removed
  - [x] Tool header change to `AGENTS.md` restored
  - [x] CodeLattice registry restored to `codelattice`

- **Failure classification:** NONE
- **Rollback action:** Not required
- **Final status:** PASS

---

## Trial #4-B — Cangjie cjgui

- **Date:** 2026-05-10 12:14-12:16 CST
- **Executor:** external AI independent retry after Run #003 fmt failure (Codex)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui
- **Language:** cangjie
- **CodeLattice HEAD:** `d2c519f`
- **Command:**
  ```
  cargo run --features tree-sitter-cangjie -- analyze --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui --language cangjie --format gitnexus-rc --strict > /tmp/codelattice-run004-cangjie-XXXXXX.json
  ```

- **Bridge JSON size:** 1,031,702 bytes
- **Stdout JSON purity:** PASS
  - First byte: `{` (`0x7b`)
  - `python3 -m json.tool /tmp/codelattice-run004-cangjie-XXXXXX.json >/dev/null`: PASS
  - No sed cleanup used

- **Tool ingestion:** SUCCESS
  - Effective command from `/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index`:
    ```
    node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-run004-cangjie-XXXXXX.json
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
  - Restored `AGENTS.md` / `CLAUDE.md`, refreshed `cjgui`, then `detect-changes --repo cjgui --scope all`: No changes detected.

- **Cleanup performed:**
  - [x] Temporary Cangjie bridge JSON deleted
  - [x] Deterministic comparison temp JSON deleted
  - [x] cangjie-GitNexus-Index `AGENTS.md` / `CLAUDE.md` header artifacts restored
  - [x] cangjie-GitNexus-Index clean except ignored `.gitnexus/`
  - [x] CodeLattice registry restored to `codelattice`

- **Failure classification:** NONE
- **Rollback action:** Not required
- **Final status:** PASS

---

## Summary

| Item | Rust (#4-A) | Cangjie (#4-B) |
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

**Overall Run #004: PASS — counted for beta criteria.**

Run #003 remains **FAIL / not counted**. Run #004 is the independent retry after the format hygiene cleanup and is the first beta-countable external AI independent PASS run.

---

## Beta Criteria Impact

| Criterion | Impact |
|-----------|--------|
| Trial count | 3/5 beta-countable PASS runs (`#001`, `#002`, `#004`) |
| Trial logs | 3/3 beta-countable PASS logs |
| External AI independent run | 1/1 PASS |
| Tool ingestion stability | PASS |
| Stdout purity | PASS |
| Endpoint integrity | PASS |
| Beta | NOT YET — remaining gaps are calendar span and two more trial runs |

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
