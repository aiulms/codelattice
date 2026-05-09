# Periodic Alpha Trial Log — Template

> **日期：** 2026-05-09
> **版本：** 1.0.0
> **类型：** Trial Log Template（空白模板，不包含伪造数据）
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Maintenance and Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)

---

## 使用说明

每次 periodic alpha trial 操作后，复制以下模板填写实际结果。填写后追加到本文档末尾，或存为独立文件 `trial-log-YYYY-MM-DD-<target>.md`。

---

## Trial Log 模板

```
### Trial #N — <简要描述>

- **Date:** YYYY-MM-DD
- **Executor:** <执行者标识（AI session / human）>
- **Target repo/path:** <分析目标路径>
- **Language:** rust / cangjie
- **Command:**
  <实际执行的完整命令>

- **Bridge JSON size:** <bytes>
- **Stdout JSON purity:** PASS / FAIL
  - 验证方法：python3 -c "import json; json.load(open('<path>'))"
  - 首字节：{ / 其他

- **Tool ingestion:** SUCCESS / FAILURE
  - Command: node <Tool path> analyze --force --experimental-rust-core-bridge-graph <path>
  - Result: <indexed successfully / error message>

- **Stats:**
  - Nodes: <N>
  - Edges: <N>
  - Symbols: <N>
  - Source files: <N>
  - Packages: <N>
  - Diagnostics: <N>

- **Quality checks:**
  - Dangling source/target: <N>（必须为 0）
  - Duplicate node IDs: <N>（必须为 0）
  - Duplicate edge triples: <N>（必须为 0）
  - Deterministic: PASS / FAIL（排除 generatedAt）
  - Quality gates (--strict): PASS / FAIL

- **Cleanup performed:**
  - [ ] 临时 bridge JSON 已删除
  - [ ] AGENTS.md / CLAUDE.md header artifact 已还原（如适用）
  - [ ] 无业务源码修改

- **Failure classification:** NONE / <类型>
  - stdout purity / dangling / duplicate / deterministic drift / adapter validation / header artifact / command authority

- **Rollback action:** NONE / <描述回滚操作>

- **Final status:** ✅ PASS / ❌ FAIL

- **Notes:**
  <补充说明、观察、待跟进项>
```

---

## 已记录的 Trial（空）

*尚无实际 trial 记录。下一次 periodic alpha trial 执行后在此追加。*
