# Better Impact Risk Reasons — Preflight

**日期：** 2026-05-13
**状态：** Ready
**前置：** `9d0b157 feat(mcp): detect changed symbols from git diff`

## 目标

增强 `codelattice_impact_preview` 和 `codelattice_production_assist` 的风险解释能力。让 AI 明确知道"为什么这个改动风险高/中/低，以及优先检查哪些调用方、文件、低置信度边和公开符号"。

## 当前状态

- `codelattice_impact_preview` 只返回 `risk` (LOW/MEDIUM/HIGH) + `reasons` (简单文本)
- `codelattice_production_assist` 只返回项目级 risk + recommendations
- AI agent 无法区分"3 个调用方但都是高置信度"和"1 个调用方但是低置信度"

## 交付物

1. `impactMetrics` — 定量指标（callerCount, downstreamCount, impactedFileCount, crossFileCount, publicSymbolCount, testFileCount, confidence edge counts）
2. `confidenceSummary` — 置信度统计（min/avg/max, high/medium/low/unknown counts）
3. `riskReasons` — 人可读风险原因（面向 AI 决策）
4. `reviewFocus` — 优先审查目标（topCallers, topCallees, topFiles, lowConfidenceEdges, publicSymbols, testFiles）
5. `production_assist` 增加 `overallRisk`, `overallRiskReasons`, `changedSymbolImpacts`, `highestRiskSymbols`, `reviewChecklist`
6. unknown hunks 进入风险解释和 checklist

## 风险说明

- risk 是 graph-based preview，不是编译器级完整证明
- low-confidence / unknown hunk 是安全信号，不是失败
- riskReasons 是给 AI 安排 review focus 用的

## 硬边界

- 只修改 CodeLattice repo
- 保持现有 MCP 字段兼容；新增字段优先，不删除旧字段
- 不新增 LLM / embedding 依赖
