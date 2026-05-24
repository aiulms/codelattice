# AI Ask + Call Chain Pack 设计

> **日期：** 2026-05-25
> **类型：** 设计文档
> **状态：** Implemented

---

## 1. 为什么不新增 codelattice_ask 工具

当前默认 AI toolset 为 6 个 facade tools。新增工具会：
- 增加所有 AI client 的 tool selection 复杂度
- 破坏现有 tool count 预期（self-test、dogfood、文档）
- ask 本质是 workflow 编排，不是独立能力

**决策：复用 `codelattice_workflow(mode=ask)` + `codelattice_symbol(mode=call_chains)`**

---

## 2. ask 意图路由

基于关键词/模式的轻量 intent routing（不接外部 LLM）：

| 关键词模式 | 意图 | 内部编排 |
|-----------|------|---------|
| 流程/怎么运行/调用链/call flow/执行路径 | explain_flow | call_chains + symbol context |
| 在哪/找/搜索/哪个函数/which symbol | find_symbol | symbol search |
| 了解项目/项目结构/入口/架构 | inspect_project | project quick |
| 如果改/删除/重命名/影响/风险 | before_edit | 路由提示（不完成完整 what-if） |

---

## 3. call_chains 输出 schema

```json
{
  "schemaVersion": "codelattice.callChains.v1",
  "target": "query string",
  "candidates": [{ "name", "file", "line", "kind" }],
  "callChains": [{
    "chain": ["sym1 -> sym2 -> sym3"],
    "depth": 3,
    "direction": "upstream",
    "confidence": 0.7,
    "confidenceReason": "static-call-graph",
    "entryFile": "src/main.rs",
    "exitFile": "src/lib.rs",
    "files": ["src/main.rs", "src/lib.rs"],
    "edgeKinds": ["CALLS", "CALLS"]
  }],
  "readFirst": [{ "kind", "path", "reason" }],
  "ambiguity": "multiple candidates found",
  "missingEvidence": [...],
  "generatedFrom": "static-call-graph",
  "analysisSemantics": { "staticAnalysis": true, ... },
  "nextActions": [...]
}
```

---

## 4. compact 策略

- `compact=true`（默认）：最多返回 8 条链、每条链最多 depth 4、不返回完整 graph 大对象
- `compact=false`：返回完整结果
- 超限时返回 `detailHint` 和 `nextActions` 指引用户获取更多

---

## 5. stop-lines

- 不执行目标项目代码
- 不做真实语义 LLM
- 不做运行时证明
- 不做全类型推断
- 所有输出标注 staticAnalysis=true
