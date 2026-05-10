# Beta Readiness Evidence Board

> **日期：** 2026-05-10
> **版本：** 1.2.0
> **类型：** 持续更新的证据看板（每次 trial 后更新）
> **关联：** [Beta Criteria Preflight](2026-05-09-beta-readiness-criteria-preflight.md)、[Go/No-Go #001](2026-05-09-beta-readiness-go-no-go-review-001.md)、[Go/No-Go #002](2026-05-10-beta-readiness-go-no-go-review-002.md)、[Go/No-Go #003](2026-05-10-beta-readiness-go-no-go-review-003.md)

---

## 当前结论

| 判定 | 状态 |
|------|------|
| **Alpha Production Trial** | **ACTIVE / PASSING** |
| **Beta** | **NOT YET** |
| **Blocker** | **None**（Run #003 format hygiene blocker 已清除） |
| **Main gap** | Evidence accumulation, calendar duration, external independent execution PASS |

---

## Evidence Table — Completed Trials

### Run #001（2026-05-09）

| Item | Value |
|------|-------|
| Commit | `ffc07e3`（后续 rename 更新路径记录） |
| Executor | AI session (Sisyphus) |
| Rust target | CodeLattice self-analysis |
| Rust result | ✅ PASS — 1,700 nodes, 2,634 edges, 0 dangling, 0 duplicate |
| Cangjie target | cangjie-GitNexus-Index/runtime/cjgui |
| Cangjie result | ✅ PASS — 903 nodes, 3,252 edges, 0 dangling, 0 duplicate |
| Stdout purity | PASS（首字节 `{`，python3 json.tool 通过） |
| Tool ingestion | SUCCESS（Rust 4,826 nodes / Cangjie 4,967 nodes） |
| Stats consistency | PASS（全部实际值与 stats 字段一致） |
| detect-changes | cjgui: No changes detected; gitnexus-rc: No changes detected |
| Failure classification | NONE |
| Document | [Run #001](2026-05-09-periodic-alpha-trial-run-001.md) |

### Run #002（2026-05-10）

| Item | Value |
|------|-------|
| Commit | `18d6408` |
| Executor | AI session (Sisyphus) |
| Rust target | CodeLattice self-analysis |
| Rust result | ✅ PASS — 1,700 nodes, 2,634 edges, 0 dangling, 0 duplicate |
| Cangjie target | cangjie-GitNexus-Index/runtime/cjgui |
| Cangjie result | ✅ PASS — 903 nodes, 3,252 edges, 0 dangling, 0 duplicate |
| Stdout purity | PASS |
| Tool ingestion | SUCCESS（Rust 4,877 nodes / Cangjie 5,017 nodes） |
| Stats consistency | PASS |
| detect-changes | codelattice: No changes detected; cjgui: No changes detected |
| Run #001 vs #002 delta | Graph stats 完全一致（0 delta），deterministic output 验证通过 |
| Failure classification | NONE |
| Document | [Run #002](2026-05-10-periodic-alpha-trial-run-002.md) |

---

### Run #003（2026-05-10）— external AI independent run, not counted

| Item | Value |
|------|-------|
| Commit | `ad795a6` |
| Executor | external AI independent run (Codex) |
| Rust target | CodeLattice self-analysis |
| Rust bridge result | PASS — 1,702 nodes, 2,635 edges, 0 dangling, 0 duplicate |
| Rust Tool ingestion | SUCCESS — 5,078 nodes / 7,510 edges / 120 clusters / 157 flows |
| Cangjie target | cangjie-GitNexus-Index/runtime/cjgui |
| Cangjie bridge result | PASS — 903 nodes, 3,252 edges, 0 dangling, 0 duplicate |
| Cangjie Tool ingestion | SUCCESS — 7,219 nodes / 14,314 edges / 75 clusters / 300 flows |
| Stdout purity | PASS（首字节 `{`，python3 json.tool 通过，无 sed） |
| Deterministic | PASS（排除 `generatedAt` 后 Rust/Cangjie 二次输出严格相等） |
| Cleanup | PASS — header artifacts restored, temp JSON deleted, registry restored to `codelattice` |
| Final verification | **FAIL** — `cargo fmt --check` failed on pre-existing tracked test formatting drift |
| Failure classification | baseline verification failure; header artifact recovered |
| Counted for Beta | **No** |
| Document | [Run #003](2026-05-10-periodic-alpha-trial-run-003.md) |
| **Format hygiene fix** | **2026-05-10：`cargo fmt` applied，2 test files reformatted。Run #003 仍 FAIL / not counted。** [Closure](2026-05-10-run003-format-hygiene-cleanup-closure.md) |

---

## Beta Criteria Progress

参照 [Beta Criteria Preflight](2026-05-09-beta-readiness-criteria-preflight.md) §2.1 的 8 项必须条件：

| # | 条件 | 要求 | 当前进度 | 状态 |
|---|------|------|---------|------|
| 1 | 多轮 periodic trial 全部 PASS | ≥ 5 次 | **2/5**（Run #001, #002；Run #003 FAIL 不计入） | ⏳ 2/5 |
| 2 | Stdout purity 无回归 | 连续 ≥ 3 周无污染 | Run #001/#002 PASS；Run #003 purity PASS 但整体不计入 | ⏳ PASSing but insufficient duration |
| 3 | Dangling/duplicate/determinism 无回归 | 连续 ≥ 3 周 0 问题 | Run #001/#002 PASS；Run #003 endpoint/deterministic PASS 但整体不计入 | ⏳ PASSing but insufficient duration |
| 4 | Tool ingestion 稳定 | 无 adapter validation failure | Run #001/#002 PASS；Run #003 ingestion PASS 但整体不计入 | ⏳ PASSing but insufficient duration |
| 5 | Failure playbook 完整 | 7 类分类 + 第一响应 | 已固化 | ✅ PASS |
| 6 | Legacy naming cleanup Phase 1 | 已完成 | 已完成 | ✅ PASS |
| 7 | Trial log 实际记录 | ≥ 3 条 | **2/3 beta-countable PASS logs**；Run #003 failure log 已记录 | ⏳ 2/3 |
| 8 | 外部 AI 独立执行 | ≥ 1 次 | **0/1 PASS**；Run #003 attempted but failed baseline | ❌ Attempted, not PASS |

**汇总：** 2 PASS + 3 PASSing-but-duration-insufficient + 1 attempted-not-counted + 0 blocker + 0 runtime FAIL

---

## Additional Evidence

| 加分项 | 状态 |
|--------|------|
| Rust+Cangjie 双语言覆盖 | ✅ 2 targets × 2 runs = 4 data points |
| Tool `detect-changes` 正常返回 | ✅ codelattice / cjgui 均正常 |
| Alpha smoke 可靠性已修复 | ✅ exit code + temp file 方案，8/8 PASS |
| Rename identity 稳定 | ✅ CodeLattice / codelattice 全链路确认 |
| Deterministic output（排除 generatedAt） | ✅ Run #001 vs #002 graph stats 完全一致 |
| External AI runbook executability | ⚠️ Run #003 bridge steps executable, but mandatory `cargo fmt --check` blocked PASS |
| Format hygiene blocker | ✅ Resolved — `cargo fmt` applied, `cargo fmt --check` now passes cleanly |

---

## Next Evidence Needed

1. **~~Resolve or explicitly triage pre-existing `cargo fmt --check` drift~~** — ✅ DONE。`cargo fmt` applied (2026-05-10)，blocker cleared。
2. **External AI independent PASS run** — Run #003 FAIL / not counted；条件 #8 仍需 1 次 PASS。应执行 Run #004。
3. **Run #004 / #005** — 建议间隔 ≥ 1 周后执行，积累时间跨度。
4. **每次 run 后更新本 board。**

---

## Explicit Non-Goals（Beta 阶段不包含）

- 不切默认工具（CodeLattice 仍是 explicit opt-in）
- 不替代 TS adapter
- 不切 WebUI / MCP 默认引擎
- 不新增语言支持
- 不重命名 Cargo package / binary
- 不突破 Rust/Cangjie stop-line
- 不新增依赖
- 不持久化 PermissionProfile
