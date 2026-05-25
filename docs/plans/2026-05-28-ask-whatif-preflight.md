# Ask Orchestration + Whatif Preflight Plan

> **日期：** 2026-05-28
> **类型：** Preflight Plan
> **状态：** Approved

---

## Execution Card

### Write Set
- `crates/cli/src/mcp_server.rs` — 新增 whatif mode + 增强 ask orchestration
- `crates/cli/tests/mcp_server.rs` — 新增 6 个测试
- `docs/mcp/ai-usage-guide.md` — 更新
- `CHANGELOG.md` — 更新

### Forbidden Set
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool/`
- open-nwe / cangjie 源码
- 新增 MCP 顶层工具
- 修改 graph schema / output contract

### Stop-lines
- 不声称 runtime proof
- 不执行目标项目代码
- 默认 toolset=6 / full=49 不变
- whatif 找不到符号时返回 structured empty，不 panic

---

## Task A: Whatif Mode

### 实现位置
`codelattice_change_review` 新增 mode=`whatif`

### 输入
- `root`, `language`, `compact`
- `change` (自然语言) 或 `symbol`+`action` 组合
- 可选 `file`

### 核心逻辑
1. 从 `change`/`symbol`/`file` 提取 targetQuery
2. 用 `build_call_chains_result` 找候选符号和调用链
3. 从调用链推导 directImpact（直接 caller/callee）和 indirectImpact（二级传播）
4. 根据 action (delete/rename/modify) 和 fan-in/fan-out 计算风险
5. 生成 safeAlternatives 和 testsToRun 建议

### 输出 schema
`codelattice.whatIf.v1`，见需求文档

---

## Task B: Ask Orchestration

### 增强位置
`codelattice_workflow(mode=ask)` 内部 `route_ask_intent`

### 变更
1. `explain_flow`：已有 call_chains 编排，补 resultsSummary
2. `locate_issue`：增加 project diagnose 步骤
3. `inspect_project`：增加 project quick 自动执行
4. `before_edit`/whatif：路由到 whatif 逻辑

### 不变
- schemaVersion 保持 `codelattice.ask.v2`
- 新增字段：`triagePlan`、`projectDigest`、`whatIf`
- 现有字段保留

---

## 测试计划

6 个测试用例，见需求文档。全部使用 `fixtures/call-resolution/c1-same-module`。

---

## 风险评估

- **Risk: medium** — 修改两个 facade handler，但都是 additive changes
- **影响面：** ask/whatif 输出格式变化，但旧字段保留
- **防守：** 先写失败测试，再实现
