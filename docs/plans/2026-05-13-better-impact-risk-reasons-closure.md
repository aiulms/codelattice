# Better Impact Risk Reasons — Closure Review

**日期：** 2026-05-13
**状态：** ✅ 完成
**提交：** (pending)
**基线：** `9d0b157`

## 交付确认

### impact_preview 新增字段

- ✅ `riskReasons` — 人可读风险原因数组
- ✅ `impactMetrics` — callerCount, downstreamCount, impactedFileCount, crossFileCount, publicSymbolCount, testFileCount, low/medium/high/unknown confidence edge counts, totalEdgesConsidered
- ✅ `confidenceSummary` — totalEdgesConsidered, high/medium/low/unknown counts, min/avg/max confidence
- ✅ `reviewFocus` — topCallers, topCallees, topFiles, lowConfidenceEdges, publicSymbols, testFiles
- ✅ `compact` parameter — 保留 risk/riskReasons/impactMetrics/confidenceSummary/reviewFocus，impactedSymbols 只保留 id/name/kind/file/line
- ✅ 旧字段完全保留：risk, reasons, impactedNodeCount, impactedSymbols, impactedNodesByKind, impactedEdgesByKind, topImpactedFiles, previewOnly, noWrites

### production_assist 新增字段

- ✅ `overallRisk` — 聚合风险等级
- ✅ `overallRiskReasons` — 整体风险原因
- ✅ `changedSymbolImpacts` — 每符号风险分解
- ✅ `highestRiskSymbols` — top 5 最高风险符号
- ✅ `reviewChecklist` — AI 可执行建议清单
- ✅ unknown hunks 进入 overallRiskReasons 和 reviewChecklist

### 测试

- 10 个新集成测试，全部通过
- 总测试数：76（从 66 增加）
- 测试覆盖：riskReasons、impactMetrics 完整字段、confidenceSummary 完整字段、compact 模式、reviewFocus、publicSymbolCount、overallRisk、reviewChecklist、changedSymbolImpacts、unknown hunks checklist

### 文档

- ✅ MCP contract v0.10.0 更新（§3.12, §3.23, changelog）
- ✅ README AI-sidecar workflow 更新
- ✅ Plan docs：preflight + closure

### 验证

- ✅ `cargo fmt --check` clean
- ✅ `cargo test --test mcp_server` 76/76 pass
- ✅ 编译无错误

## 未触碰

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool
- 真实项目（CoolMallArkTS / harmony-utils / HarmonyOS-Examples）
- AI client 配置
- Cargo package name
