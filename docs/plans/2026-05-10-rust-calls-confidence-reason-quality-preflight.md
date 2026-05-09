# Rust CALLS Confidence/Reason Quality Preflight

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Preflight（bounded quality improvement）
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Beta Evidence Board](2026-05-10-beta-readiness-evidence-board.md)

---

## 一、目标

固化 Rust CALLS edge confidence/reason 质量合同。不追求大规模新增解析能力，重点是：
1. 确认现有 confidence/reason 矩阵与实际行为一致
2. 确认 fixture 预期值正确覆盖各 call form
3. 补齐文档矩阵
4. 如发现明显不一致，做最小修复

---

## 二、现有 Confidence/Reason 矩阵

来源：`crates/project-model/src/model.rs` CallResolutionReason + `calls.rs` 赋值点

| # | Call Form | Confidence | Reason | Fixture |
|---|-----------|-----------|--------|---------|
| 1 | Same-module exact | 0.90 | call-same-module-resolved | c1-same-module |
| 2 | Import-resolved | 0.85 | call-import-resolved | c2-import-binding |
| 3 | Same-file unique name | 0.70 | call-same-file-unique-name | sf1-unique-helper |
| 4 | crate:: path | 0.80 | call-crate-path-resolved | c3-crate-path |
| 5 | self:: path | 0.80 | call-self-path-resolved | c4-self-path |
| 6 | super:: path | 0.80 | call-super-path-resolved | c5-super-path |
| 7 | Associated fn (unique) | 0.75 | call-associated-fn-resolved | c6-associated-fn |
| 8 | Associated fn (multi) | 0.70 | call-associated-fn-resolved | c15-associated-function-disambiguation |
| 9 | Same-crate resolved | 0.80 | call-same-crate-resolved | c13-cross-file-same-crate |
| 10 | Method name blind | 0.65 | call-method-name-resolved | c7-method-call |
| 11 | Known enum constructor | 0.80 | call-known-enum-constructor | call-enum-filter |
| 12 | External crate classified | 0.60 | call-external-crate-classified | c10-external-crate |
| 13 | External crate path (std) | 0.80-0.85 | call-external-crate-path-resolved | c10-external-crate |
| 14 | Stdlib trait method | 0.55 | call-stdlib-trait-method-resolved | c11-receiver-type |
| 15 | Receiver type method | 0.65 | call-receiver-type-method-resolved | c11-receiver-type, c12-let-constructor-method |
| 16 | Wildcard disambiguation | 0.80 | call-same-crate-resolved | c14-wildcard-disambiguation |
| 17 | Module path | 0.80 | call-module-path-resolved | call-module-path |
| 18 | Unresolved | 0.00 | call-target-unresolved | c9-method-ambiguous |
| 19 | Ambiguous | 0.00 | call-target-ambiguous | sf2-duplicate-name |

---

## 三、Fixture 覆盖分析

24 个 call-resolution fixture 全部有 `expected-calls.json`，通过 `project_model_call_expected_compare.rs` 自动比较 confidence（阈值）和 reason（精确匹配）。

| Fixture | Call Form Tested | Has expected-calls.json | Calls Count |
|---------|-----------------|------------------------|-------------|
| c1-same-module | same-module | ✅ | 2 |
| c2-import-binding | import-resolved | ✅ | 1 |
| c3-crate-path | crate:: | ✅ | 1 |
| c4-self-path | self:: | ✅ | 1 |
| c5-super-path | super:: | ✅ | 1 |
| c6-associated-fn | associated fn | ✅ | 2 |
| c7-method-call | method blind | ✅ | 2 |
| c8-method-resolution | method resolution | ✅ | 3 |
| c9-method-ambiguous | ambiguous | ✅ | 1 |
| c10-external-crate | external std/core | ✅ | 8 |
| c11-receiver-type | receiver type method | ✅ | 14 |
| c12-let-constructor-method | let constructor | ✅ | 14 |
| c13-cross-file-same-crate | same-crate | ✅ | 2 |
| c14-wildcard-disambiguation | wildcard import | ✅ | 2 |
| c15-associated-function-disambiguation | associated fn disambiguation | ✅ | 4 |
| c16-crate-associated-fn | crate associated fn | ✅ | 3 |
| call-enum-filter | enum constructor | ✅ | 7 |
| call-module-path | module path | ✅ | 2 |
| sf1-unique-helper | same-file unique | ✅ | 1 |
| sf2-duplicate-name | ambiguous same-file | ✅ | 1 |
| sf3-method-ignored | method ignored | ✅ | 1 |
| sf4-exact-priority | priority | ✅ | 1 |
| sf5-cross-module-unique | cross-module unique | ✅ | 1 |
| sf6-inline-module-flat | inline module flat | ✅ | 1 |

---

## 四、缺口分析

### 4.1 已有充分覆盖（不需要新增）

- Same-module exact (c1): ✅ 2 calls, expected JSON with confidence=0.90
- Import-resolved (c2): ✅ 1 call, expected JSON with confidence=0.85
- crate:: path (c3): ✅ 1 call, expected JSON with confidence=0.80
- self:: path (c4): ✅ 1 call, expected JSON with confidence=0.80
- super:: path (c5): ✅ 1 call, expected JSON with confidence=0.80
- Associated fn (c6): ✅ 2 calls, expected JSON
- Method blind (c7): ✅ 2 calls
- External crate (c10): ✅ 8 calls, covers both classified and path-resolved
- Enum constructor (call-enum-filter): ✅ 7 calls
- Same-file unique (sf1): ✅ 1 call
- Cross-file same-crate (c13): ✅ 2 calls
- Wildcard disambiguation (c14): ✅ 2 calls

### 4.2 文档缺口（需要补齐）

- **无 Rust confidence/reason 矩阵文档**：QUALITY.md 只覆盖 Cangjie quality gates，Rust CALLS 矩阵只存在于代码注释和 reason enum 定义中
- **需要产出**：一个 Rust CALLS confidence matrix 文档或在 closure 中记录

### 4.3 可补强的测试断言（低优先级）

- 部分 fixture 只有 1 个 call entry，可以增加更多 boundary case
- 但现有 expected-calls.json + call_expected_compare 已自动验证 confidence/reason
- **结论**：不强制新增 fixture，只补文档矩阵

---

## 五、本 Slice 的 Write Set

| 操作 | 文件 | 说明 |
|------|------|------|
| 新增 | `docs/plans/2026-05-10-rust-calls-confidence-reason-quality-preflight.md` | 本文档 |
| 可能新增 | `docs/architecture/rust-calls-confidence-matrix.md` | Rust CALLS 矩阵文档（可选，也可在 closure 中记录） |
| 新增 | `docs/plans/2026-05-10-rust-calls-confidence-reason-quality-closure.md` | Closure 文档 |

**不修改的文件：**
- `crates/project-model/src/calls.rs` — 除非发现实际 confidence/reason 与 expected 不一致
- `crates/cli/src/rust_bridge.rs` — bridge 已正确传递 confidence/reason
- `fixtures/call-resolution/*/expected-calls.json` — 现有预期值已与代码对齐

---

## 六、Stop-line 确认

- ❌ No new resolution strategies
- ❌ No type inference
- ❌ No trait solving
- ❌ No macro expansion
- ❌ No global fallback for resolution rate
- ❌ No cargo metadata execution
- ✅ Documentation of existing behavior
- ✅ Minimal fix if actual ≠ expected (none found so far)

---

## 七、结论

Rust CALLS confidence/reason 质量合同现状**良好**：
- 19 种 call form 全部有明确的 confidence 和 reason 定义
- 24 个 fixture 全部有 expected-calls.json，自动验证
- `project_model_call_expected_compare.rs` 已实现 confidence 阈值比较 + reason 精确匹配
- `bridge_roundtrip.rs` 已验证 bridge 输出保留 confidence/reason

**本 slice 主要产出：** 文档矩阵 + closure 确认。不需要代码修复。
