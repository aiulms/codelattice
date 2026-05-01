# No-edge 策略

> **日期：** 2026-05-01
> **类型：** 策略决策
> **状态：** 从 GitNexus-RC 冻结

---

## 策略声明

**No-edge 优先于 false edge。**

当 resolver 无法自信地确定关系（import、call、ownership、resolution）时，必须：
1. 发出带文档化 reason 的 low-confidence edge，或
2. 如果 confidence 会 ≤ 0.50，则**完全不发出 edge**

---

## 理由

False positive edges 污染知识图谱并误导 impact analysis。当 symbol 被修改时：
- False positive CALLS edges 导致不必要的 CRITICAL 风险警告
- False positive IMPORTS edges 导致不必要的 rebuilds
- False positive ownership edges 导致不正确的 module attribution

Low-confidence edges（0.35-0.70）在以下情况可接受：
- 解析路径已知但非 compiler-verifiable
- 语言特定 heuristic 解释了不确定性
- Reason code 诚实记录了局限

---

## 在 ProjectModel 中的应用

### Source Ownership

| 场景 | 策略 | Reason Code |
|---------|--------|-------------|
| Source 匹配多个 packages | Nearest wins，confidence 降级 | `nearest-cargo-root-resolved` |
| Source 在任何 package 之外 | 无 high-confidence edge | `cargo-root-missing` |
| 歧义 root ownership | No edge | `cargo-root-ambiguous` |

### Root Resolution

| 场景 | 策略 | Reason Code |
|---------|--------|-------------|
| Virtual workspace root | 不作为 crate root | `no-edge` |
| Nested package source | 不解析到 outer member | `outer-workspace-member-not-owner` |
| Package 外的 source | 无 `crate::` 解析 | `cargo-root-missing` |

---

## 与 expectedAbsence 的关系

Fixtures 中的 `expectedAbsence` 字段明确记录了**不应该存在**的内容：

```json
{
  "type": "noRootPackageOwnership",
  "description": "backend source should not resolve to root package",
  "sourcePath": "backend/src/api/handlers.rs",
  "forbiddenPackage": "root-app"
}
```

这与"因为无法确定所以 no edge"不同：
- `expectedAbsence` = **主动禁止** by policy
- 因不确定性 no edge = **被动** because confidence too low

两者都防护 false positives，但 `expectedAbsence` 更强。

---

## 为什么这是 Rust-core 的核心

No-edge 策略是 Rust-core 的**基础**，因为：

1. **信任** — 用户依赖 impact analysis 找到真实 breakage
2. **可审计性** — Low-confidence edges 必须有文档化的 reason codes
3. **Fixture 覆盖** — Absence 断言防护回归
4. **语言中立** — 策略同等适用于 Rust、Cangjie 等

---

## 来源

GitNexus-RC RISK_LEDGER.md § "Graph schema 过早扩张" 和 rust-core rebuild preflight 中的 "No-edge preferred over false edge" 策略。
