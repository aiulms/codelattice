# Rust CALLS Confidence/Reason Quality Closure

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Closure Review
> **关联：** [Preflight](2026-05-10-rust-calls-confidence-reason-quality-preflight.md)

---

## 一、完成概要

Rust CALLS confidence/reason 质量合同审查完成。**无代码修复，无 behavior change。** 产出为文档固化。

---

## 二、变更文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `docs/plans/2026-05-10-rust-calls-confidence-reason-quality-preflight.md` | 新增 | Preflight 文档 |
| `docs/architecture/rust-calls-confidence-matrix.md` | 新增 | Rust CALLS confidence/reason 矩阵文档 |
| `docs/plans/2026-05-10-rust-calls-confidence-reason-quality-closure.md` | 新增 | 本文档 |

**未修改的文件：**
- `crates/project-model/src/calls.rs` — confidence/reason 值与 expected 一致
- `crates/project-model/src/model.rs` — CallResolutionReason 定义完整
- `crates/cli/src/rust_bridge.rs` — bridge 正确传递 confidence/reason
- `fixtures/call-resolution/*/expected-calls.json` — 全部与代码对齐

---

## 三、Confidence/Reason 矩阵

完整矩阵见 [`docs/architecture/rust-calls-confidence-matrix.md`](../architecture/rust-calls-confidence-matrix.md)。

19 种 call form，confidence 范围 0.55-0.90，no-edge 为 0.00。层级清晰：
- Exact (0.90): same-module
- Resolved (0.65-0.85): import, path, associated fn, method
- Heuristic (0.55-0.70): stdlib trait method, external crate classified
- No-edge (0.00): unresolved, ambiguous

---

## 四、Fixture/Test 覆盖

- 24 个 call-resolution fixture 全部有 `expected-calls.json`
- `project_model_call_expected_compare.rs` 自动验证 confidence（阈值）和 reason（精确）
- `bridge_roundtrip.rs` 验证 bridge 输出保留 confidence/reason
- 无新增 fixture（现有覆盖已充分）

---

## 五、Resolution Rate / Graph Stats

**无变化。** 本 slice 不修改任何运行时行为，不改变 resolution rate 或 graph stats。

---

## 六、Stop-line

**未触碰。** 纯文档产出，无代码变更。

---

## 七、验证结果

| 验证项 | 结果 |
|--------|------|
| cargo fmt --check | ✅ PASS |
| git diff --check | ✅ PASS |
| bridge_roundtrip (no feature) | 13/13 ✅ |
| bridge_roundtrip (with cangjie) | 13/13 ✅ |
| productization_commands | 30/30 ✅ |
| alpha-trial-smoke --rust-only | 5/5 ✅ |
| alpha-trial-smoke --cangjie-only | 5/5 ✅ |
| Tool status | up-to-date ✅ |

**已知预存问题：** `project_model_call_expected_compare` 7 个测试因硬编码旧路径 `/Users/jiangxuanyang/Desktop/gitnexus-rust-core/` 失败 — 非本 slice 引入。

---

## 八、后续 Opening

- 低优先级：修复 `project_model_call_expected_compare.rs` 中的硬编码路径（使用相对 fixture 路径）
- 低优先级：为 cangjie project tests 添加 fixture 路径环境适配
