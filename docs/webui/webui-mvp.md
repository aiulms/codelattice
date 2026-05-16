# CodeLattice WebUI — MVP Specification

> **日期：** 2026-05-16
> **关联文档：** [README.md](./README.md)（readiness 总览）、[webui-snapshot-contract.md](./webui-snapshot-contract.md)（数据契约）

---

## 1. MVP Scope Definition

### 1.1 In Scope

| 能力 | 描述 | 优先级 |
|------|------|--------|
| Snapshot 生成 | `webui-snapshot.sh` 输出符合 contract 的 JSON | P0 |
| 5 个视图数据 | Dashboard / Explore / Impact / Cleanup / Release Review | P0 |
| Fixture snapshots | Rust + TypeScript 示例 snapshot | P0 |
| Smoke 验证 | `webui-snapshot-smoke.sh` 自动验证 | P0 |
| Caution 渲染规则 | 每个 view 的 stop-line / caution 展示规范 | P0 |
| Contract stability | `CodeLatticeWebSnapshotV1` 字段稳定性标注 | P0 |

### 1.2 Out of Scope (MVP)

| 能力 | 原因 | 未来考虑 |
|------|------|----------|
| 前端渲染 | 本轮只准备数据契约 | Tauri/Electron/纯 HTML |
| 实时更新 | snapshot 是静态的 | WebSocket / polling |
| 用户交互 | 搜索/过滤/展开由未来前端实现 | - |
| 认证/权限 | 本地工具不需要 | - |
| 跨项目对比 | 需要 multi-project 支持 | v2 |
| 历史趋势 | 需要 snapshot 存储 + diff | v2 |

---

## 2. View-by-View Specification

### 2.1 Dashboard View

**Layout suggestion:**
```
┌──────────────────────────────────────────────────┐
│ Project: <name>  Language: <lang>  Analyzed: <t> │
│ ⚠️ Static analysis only — not compiler verified  │
├────────────┬─────────────┬───────────────────────┤
│ Quality    │ Stats       │ Risk Overview         │
│ Gates       │             │                       │
│ ✅ dup.. 7  │ 838 symbols │ 🟢 Overall: LOW      │
│ ✅ dang.. 0 │ 50 files    │ 3 hotspots           │
│ ✅ diag.. 1 │ 3 packages  │ 2 dense files        │
├────────────┴─────────────┴───────────────────────┤
│ Entry Points (3)   Hotspots (top 5)              │
│ • main             • process_request (fan-out:12)│
│ • lib.rs::init     • parse_config  (fan-in:8)    │
│ • tests::all       │                              │
└──────────────────────────────────────────────────┘
```

**Required data sections in snapshot:** `summary`, `quality`, `insights.riskMap`

**Stability labels:**
- `summary.*` — stable
- `quality.gates[]` — stable
- `quality.qualityMetrics` — stable (fields may expand)
- `insights.entryPoints` — heuristic
- `insights.hotspots` — heuristic

---

### 2.2 Explore View

**Layout suggestion:**
```
┌──────────────────────────────────────────────────┐
🔍 Search: [____________]  Kind: [All ▾]          │
├──────────────────────────────────────────────────┤
│ Selected: helper (function)                       │
│ File: src/lib.rs:1-3                             │
│                                                    │
│ ┌─ Source Snippet ──────────────────────────┐   │
│ │ 1 pub fn helper() -> i32 {                │   │
│ │ 2     42                                  │   │
│ │ 3 }                                       │   │
│ └───────────────────────────────────────────┘   │
│                                                    │
│ Callers (1)          Callees (0)                   │
│ • main_fn [0.90]     (none)                        │
│   reason: call-same-module-resolved               │
└──────────────────────────────────────────────────┘
```

**Required data sections:** `explore.symbols[]`, `explore.searchMeta`

**Stability labels:**
- `symbols[].id/name/kind/file/line` — stable
- `symbols[].sourceSnippet` — stable (may be null)
- `symbols[].incomingEdges/outgoingEdges` — stable
- `edges[].confidence/reason` — stable

---

### 2.3 Impact View

**Layout suggestion:**
```
┌──────────────────────────────────────────────────┐
📊 Impact Analysis: helper                          │
│ Risk: 🟢 LOW                                     │
│ Reason: Small blast radius, few callers           │
├──────────────────────────────────────────────────┤
│ Metrics                                           │
│ Callers: 1  Files: 1  Cross-file: 0              │
│ Confidence: min=1.00 avg=1.00 max=1.00           │
├──────────────────────────────────────────────────┤
│ Affected Files                                    │
│ • src/lib.rs (2 symbols impacted)                 │
├──────────────────────────────────────────────────┤
│ Review Focus                                      │
│ □ Inspect direct caller: main_fn                  │
│ □ No low-confidence edges found                   │
└──────────────────────────────────────────────────┘
```

**Required data sections:** `impact.entries[]`

**Stability labels:**
- `impact[].symbol/risk/riskLevel` — stable
- `impact[].riskReasons[]` — stable
- `impact[].impactMetrics` — stable
- `impact[].confidenceSummary` — stable
- `impact[].reviewFocus` — preview (structure may refine)

---

### 2.4 Cleanup View

**Layout suggestion:**
```
┌──────────────────────────────────────────────────┐
🧹 Cleanup Candidates                               │
│ ⚠️ NOT deletion proof — verify before removing   │
├──────────────┬──────────────┬────────────────────┤
│ Dead Code    │ Unreachable  │ External API        │
│ 5 symbols    │ 3 files      │ 12 surface symbols  │
│ (3 high)     │ (1 high)     │ caution: medium     │
├──────────────┼──────────────┼────────────────────┤
│ Framework Entries (8)                            │
│ • route: getUser (express/nextjs)                │
│ • callback: onData (event handler)               │
└──────────────┴──────────────┴────────────────────┘
```

**Required data sections:** `cleanup.deadCodeCandidates`, `cleanup.reachability`, `cleanup.externalApiSurface`, `cleanup.frameworkEntries`

**Stability labels:**
- `cleanup.deadCodeCandidates.summary` — stable
- `cleanup.deadCodeCandidates.candidates[]` — heuristic
- `cleanup.reachability.entryPoints[]` — heuristic
- `cleanup.externalApiSurface.summary` — heuristic
- `cleanup.frameworkEntries.hints[]` — heuristic

**Critical cautions (must render prominently):**
1. "Static analysis cannot prove code is unused"
2. "Public/exported APIs may have external consumers"
3. "Framework callbacks/routes may be invoked at runtime"
4. "Dynamic dispatch (reflection, plugins, registry) hides callers"

---

### 2.5 Release Review View

**Layout suggestion:**
```
┌──────────────────────────────────────────────────┐
📋 Release Review                                   │
│ ⚠️ Static review only — run project tests separately │
├──────────────┬──────────────┬────────────────────┤
│ Breaking     │ Consistency   │ Config/Examples     │
│ Change       │ Review        │ Review              │
│ Risk: medium │ 3 stale docs │ 2 stale refs        │
│ 2 changed    │ 1 missing    │                     │
│ public API   │ test         │                      │
├──────────────┴──────────────┴────────────────────┤
│ Checklist (P0 first)                              │
│ ☐ Verify external consumers of changed public API │
│ ☐ Update README for removed function              │
│ ☐ Add unit test for new parameter validation      │
└──────────────────────────────────────────────────┘
```

**Required data sections:** `releaseReview.breakingChange`, `releaseReview.consistency`, `releaseReview.configExamples`

**Stability labels:**
- `releaseReview.breakingChange.compatibilityRisk` — heuristic
- `releaseReview.breakingChange.reviewChecklist[]` — preview
- `releaseReview.consistency.*` — heuristic
- `releaseReview.configExamples.*` — heuristic

---

## 3. Caution Rendering Spec

所有视图必须渲染的全局 caution：

```html
<div class="codelattice-caution-banner">
  <strong>⚠️ Static Analysis Only</strong>
  Results are based on source-code graph analysis.
  This is <strong>not</strong> compiler-verified, runtime-tested,
  or coverage-proven. Use findings as investigation leads,
  not as proof of safety or deletion-worthiness.
</div>
```

Per-view caution badges:

| Context | Badge | When shown |
|---------|-------|------------|
| Low confidence edge (< 0.6) | 🟡 Low confidence | Any CALLS edge with conf < 0.6 |
| Public API dead code candidate | 🔴 Public API | Dead code candidate is `pub`/exported |
| Framework entry | 🔵 Framework hint | Symbol appears in framework_entry_hints |
| Unknown hunk | ⚪ Unknown | Diff region not mapped to any symbol |
| dangling edge | 🟠 Dangling | Graph edge references non-existent node |

---

## 4. Responsive Considerations

- Mobile: 单列布局，table 改 card list
- Tablet: 双列（Dashboard stats + quality side by side）
- Desktop: 全宽三列（Explore + callers + callees）
- Print: 简洁版，去掉交互元素

（这些是未来前端的参考，本轮不实现。）

---

## 5. Accessibility Notes (Future)

- 所有 caution banner 必须在 `<section role="alert">` 中
- 风险色不能是唯一区分符（需文字标签）
- 表格需要 `<th scope="col">` / `<th scope="row">`
- 图表需要 `<figcaption>` + alt text
