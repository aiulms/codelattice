# Beta Readiness Criteria Preflight

> **日期：** 2026-05-09
> **版本：** 1.0.0
> **类型：** Preflight — 定义 Alpha → Beta 升级条件和 Go/No-Go checklist
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Maintenance and Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)

---

## 一、当前 Alpha Ready 事实

| 项目 | 状态 |
|------|------|
| 支持语言 | Rust + Cangjie only |
| 接入方式 | Explicit opt-in（`--format gitnexus-rc` + `--experimental-rust-core-bridge-graph`） |
| Tool bridge ingestion | ✅ 正常工作 |
| Validator 策略 | Fail-closed，未放宽 |
| Dangling endpoints | 0 |
| Duplicate nodes/edges | 0 |
| Deterministic output | PASS（排除 generatedAt） |
| Stdout purity | PASS（从第 1 字节合法 JSON） |
| Quality gates | 7/7 Rust, 6/6 Cangjie |
| Regression tests | 440+ 全通过 |
| 操作手册 | Runbook + Failure Playbook + Trial Log Template |
| Legacy cleanup Phase 1 | 已完成 |

---

## 二、Beta 候选条件

### 2.1 必须满足

| 条件 | 当前 | 要求 |
|------|------|------|
| 多轮 periodic real-project trial 稳定 | 2 次（初始验证） | ≥ 5 次真实项目 trial 全部 PASS |
| Stdout purity 无回归 | ✅ | 连续 ≥ 3 周无 stdout 污染 |
| Dangling/duplicate/determinism 无回归 | ✅ | 连续 ≥ 3 周 0 问题 |
| Tool ingestion 稳定 | ✅ | 无 adapter validation failure |
| Failure playbook 完整 | ✅ | 7 类失败分类 + 第一响应 |
| Legacy naming cleanup Phase 1 完成 | ✅ | 已完成 |
| Trial log 有实际记录 | 尚无 | ≥ 3 条实际 trial log |
| 至少一个外部执行 AI 成功按 runbook 操作 | 尚无 | 至少 1 次独立执行 |

### 2.2 加分项（非阻塞）

- 更多真实项目覆盖（≥ 3 个 Rust + ≥ 2 个 Cangjie）
- Tool `detect-changes` 功能在真实项目中正常返回
- Periodic smoke 自动化（CI 或 cron）

---

## 三、Beta 不包含

| 不包含 | 说明 |
|--------|------|
| 默认替换 TS adapter | Rust-core 仍是 explicit opt-in |
| WebUI 默认切换 | 不涉及 |
| MCP 默认切换 | 不涉及 |
| 多语言扩张 | 第一阶段仍仅 Rust + Cangjie |
| 完整 trait solving | Rust stop-line |
| 宏展开 / proc-macro | Rust stop-line |
| Full cfg evaluator | Rust stop-line |
| 完整 Cangjie method dispatch / interface solving | Cangjie stop-line |

Beta 只是从 "alpha opt-in trial" 升级为 "beta opt-in trial"，不改变功能范围。

---

## 四、Go / No-Go Checklist

在考虑 Alpha → Beta 升级时，逐项确认：

- [ ] ≥ 5 次真实项目 periodic trial 全部 PASS
- [ ] 连续 ≥ 3 周无 stdout purity 回归
- [ ] 连续 ≥ 3 周 0 dangling / 0 duplicate
- [ ] 连续 ≥ 3 周 deterministic output PASS
- [ ] Tool ingestion 无 adapter validation failure
- [ ] 至少 3 条实际 trial log 记录
- [ ] 至少 1 次外部执行 AI 独立按 runbook 操作成功
- [ ] Failure playbook 无未覆盖的失败模式
- [ ] `--format gitnexus-rc` 和 `--experimental-rust-core-bridge-graph` 仍正常工作
- [ ] 未切默认工具、未替代 TS adapter
- [ ] 所有 regression tests 通过

**判定规则：**
- 全部 ✅ → 可标记 Beta Readiness
- 任何一项 ❌ → 保持 Alpha，修复后重新评估

---

## 五、下一轮建议

1. **Run periodic alpha trial**：在 Rust-core 自身 + Cangjie cjgui index checkout 各跑一次完整 trial
2. **Fill trial log template**：用 [Trial Log Template](2026-05-09-periodic-alpha-trial-log-template.md) 记录结果
3. **评估 beta criteria**：积累足够 trial log 后，按 Go/No-Go Checklist 判定
4. **不需要新功能**：Beta 是信任等级升级，不是功能扩张

---

## 六、相关文档

- [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)
- [Maintenance and Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)
- [Periodic Alpha Trial Log Template](2026-05-09-periodic-alpha-trial-log-template.md)
- [Public Identity and Legacy Command Cleanup Plan](2026-05-09-public-identity-and-legacy-command-cleanup-plan.md)
