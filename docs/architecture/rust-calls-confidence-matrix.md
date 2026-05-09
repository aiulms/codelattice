# Rust CALLS Confidence/Reason Matrix

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** Policy Reference
> **状态：** 已固化
>
> **⚠️ 重要说明：** 本文档是策略参考（policy reference），实际代码行为以 `crates/project-model/src/calls.rs` 和 `crates/project-model/src/model.rs` 中的实现为准。

---

## 一、目的

本文档固化 Rust CALLS edge 的 confidence/reason 质量合同，作为以下场景的唯一参考：

1. **开发阶段** — 新增 call form 时，确定应分配的 confidence 和 reason
2. **Code Review** — 检查 PR 中的 confidence/reason 是否符合既定策略
3. **Debug** — 用户报告 call edge 质量问题时，快速定位预期行为
4. **测试** — 编写 `expected-calls.json` 时，确认阈值和 reason code

---

## 二、Confidence 分层

| Tier | Confidence 范围 | 含义 | 典型场景 |
|------|----------------|------|---------|
| **Exact** | 0.90 - 1.00 | 语法级精确事实 | 同模块函数、已知枚举构造器 |
| **Resolved** | 0.65 - 0.85 | 通过名称解析成功 | import 绑定、路径解析、接收者类型 |
| **Heuristic** | 0.35 - 0.70 | 语言启发式推断 | 方法名盲匹配、外部 crate 分类 |
| **No-edge** | 0.00 | 不生成 edge | 未解析、歧义目标 |

---

## 三、完整矩阵

### 3.1 矩阵表

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

### 3.2 按 Confidence 排序

| Confidence | Call Forms |
|-----------|-----------|
| 0.90 | Same-module exact |
| 0.85 | Import-resolved |
| 0.80-0.85 | External crate path (std) |
| 0.80 | crate:: path, self:: path, super:: path, Same-crate resolved, Known enum constructor, Wildcard disambiguation, Module path |
| 0.75 | Associated fn (unique) |
| 0.70 | Same-file unique name, Associated fn (multi) |
| 0.65 | Method name blind, Receiver type method |
| 0.60 | External crate classified |
| 0.55 | Stdlib trait method |
| 0.00 | Unresolved, Ambiguous |

---

## 四、No-edge Policy

以下情况**不生成 CALLS edge**（confidence = 0.00）：

| 场景 | Reason | 说明 |
|------|--------|------|
| 目标未解析 | call-target-unresolved | 找不到函数定义，如未导入的外部函数 |
| 目标歧义 | call-target-ambiguous | 多个候选目标，无法确定唯一目标 |

---

## 五、Fixture 覆盖摘要

24 个 call-resolution fixture 全部有 `expected-calls.json`，通过 `project_model_call_expected_compare.rs` 自动验证：

- **confidence** — 阈值比较（实际值 ≥ 预期值）
- **reason** — 精确字符串匹配

| Fixture | Call Form | Calls |
|---------|-----------|-------|
| c1-same-module | same-module | 2 |
| c2-import-binding | import-resolved | 1 |
| c3-crate-path | crate:: | 1 |
| c4-self-path | self:: | 1 |
| c5-super-path | super:: | 1 |
| c6-associated-fn | associated fn | 2 |
| c7-method-call | method blind | 2 |
| c8-method-resolution | method resolution | 3 |
| c9-method-ambiguous | ambiguous | 1 |
| c10-external-crate | external std/core | 8 |
| c11-receiver-type | receiver type method | 14 |
| c12-let-constructor-method | let constructor | 14 |
| c13-cross-file-same-crate | same-crate | 2 |
| c14-wildcard-disambiguation | wildcard import | 2 |
| c15-associated-function-disambiguation | associated fn disambiguation | 4 |
| c16-crate-associated-fn | crate associated fn | 3 |
| call-enum-filter | enum constructor | 7 |
| call-module-path | module path | 2 |
| sf1-unique-helper | same-file unique | 1 |
| sf2-duplicate-name | ambiguous same-file | 1 |
| sf3-method-ignored | method ignored | 1 |
| sf4-exact-priority | priority | 1 |
| sf5-cross-module-unique | cross-module unique | 1 |
| sf6-inline-module-flat | inline module flat | 1 |

**总计：** 24 fixtures，覆盖全部 19 种 call form。

---

## 六、Stop-line 确认

本文档只描述**现有行为**，不涉及以下超出范围的内容：

- ❌ 不新增解析策略
- ❌ 不进行类型推断
- ❌ 不进行 trait solving
- ❌ 不展开宏
- ❌ 不使用全局 fallback 提升解析率
- ❌ 不执行 cargo metadata

---

## 七、变更历史

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-10 | 1.0.0 | 初始版本，从 preflight 提取并固化 19 种 call form 的 confidence/reason 矩阵 |

---

## 八、来源

- `crates/project-model/src/model.rs` — `CallResolutionReason` enum 定义
- `crates/project-model/src/calls.rs` — confidence 赋值点
- `docs/plans/2026-05-10-rust-calls-confidence-reason-quality-preflight.md` — 质量预检文档
