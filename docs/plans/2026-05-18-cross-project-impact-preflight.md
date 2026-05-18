# Cross-Project Impact Analysis — Preflight

日期：2026-05-18
状态：preflight → implementation

## 目标

基于已有的 Workspace Cross-Project Graph，新增跨项目影响分析能力。
回答："如果我改了这个子项目 / 配置 / 脚本 / package / path，会影响 workspace 里的哪些项目？"

## 前置依赖

- Workspace Graph Pack（commit af03151）：已实现 workspace graph build + API + insights integration
- Node kinds: workspace / project / config / script / workflow / unsupported
- Edge kinds: contains / depends_on / imports / script_refs / config_refs / adjacent_to / unsupported_boundary

## Impact Schema (CodeLatticeCrossProjectImpactV1)

### Target Resolution

输入优先级：
1. exact node id → confidence 1.0
2. exact projectId → confidence 1.0
3. exact snapshotId → project node → confidence 0.95
4. exact relative path / suffix path → confidence 0.90 / 0.75
5. exact label → confidence 0.65
6. fuzzy contains → confidence 0.45
7. unknown → confidence 0.0

多候选时：返回 resolutionCandidates，取最高 confidence；并列时降级并在 cautions 说明。

### Traversal 策略

- BFS on graph edges，direction: downstream / upstream / both
- maxDepth 默认 3，edgeCount cap 1000，path cap 100
- Edge 语义：
  - downstream: target → outgoing edges（target 变动影响谁依赖它）
  - upstream: incoming edges → target（target 依赖谁）
  - contains: workspace→project 可从 workspace 看全部；project 内部不太扩散
  - depends_on/imports: source references target → 如果 target 变，incoming 的 source 受影响
  - script_refs/config_refs: 脚本/配置变动 → 引用它的 project/workflow 受影响
  - adjacent_to: 不作为强影响，仅 low-confidence review lead
  - unsupported_boundary: 不扩散为确定影响，仅进入 unsupportedBoundaries

### Path Confidence

- path confidence = min(edge confidence along path)
- affected node confidence = max confidence among paths
- 含 adjacent_to / unsupported_boundary 的 path confidence 降级

### Risk Scoring

| Level | Criteria |
|-------|----------|
| critical | affectedProjects >= 8，或 workflow >= 3 + script/config refs，或 shared config + unsupported boundary |
| high | affectedProjects >= 4，或 workflow > 0，或 unsupportedBoundary >= 3，或 low confidence + large fanout |
| medium | affectedProjects >= 2，或 config/script > 0，或有 unsupported boundary |
| low | 单项目或只有 contains/adjacent 线索 |
| unknown | target resolution failed or no graph |

### Confidence Summary

- high: majority path confidence >= 0.75
- medium: >= 0.5
- low: < 0.5 or ambiguous target
- unknown: no resolved target

## Runner API

1. `GET /api/workspace/impact?runId=<id>&projectId=<pid>&direction=both`
2. `POST /api/workspace/impact` with body: `{workspaceRunId, target, direction, maxDepth, includeUnsupported, limit}`

Error cases: missing runId, missing target, run not found, graph build failed, target unresolved, invalid direction.

## Insights Integration

Enhance `/api/workspace/insights` with `crossProjectImpactHints`:
- highFanoutProjects
- sharedScripts / sharedConfigs
- unsupportedBoundaryProjects
- suggestedImpactTargets

## WebUI 最小展示

- Impact panel in Workspace tab: target input, direction select, run button
- Summary cards: affected projects, configs, scripts, workflows
- Affected tables: projects + assets
- Unsupported boundaries list
- riskReasons + reviewChecklist
- "Copy Impact Summary for AI" button
- 12 i18n keys (zh + en)

## Smoke Tests

- `webui-workspace-impact-smoke.sh` — 16 steps
- Fixture: rust-core + ts-ui + scripts + CI + Dockerfile + Makefile + unsupported (same as graph smoke)

## Stop-lines

- 不执行目标项目代码
- 不改变 workspace graph schema
- 不改变 single snapshot schema
- 不输出"会破坏"断言，只输出"可能影响 / review lead"
- target resolution 失败时返回 structured error，不 crash
- 不添加图谱视觉引擎
