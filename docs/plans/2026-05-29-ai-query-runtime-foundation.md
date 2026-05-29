# AI Query Runtime Foundation

> **状态**: Direction Anchor / Execution Context · **日期**: 2026-05-29  
> **目的**: 在上下文被压缩、多人/多 AI 接力、功能继续扩展时，明确 CodeLattice 当前优化到底要解决什么，防止后续工作偏成零散补丁或单纯堆功能。

---

## 1. 一句话定位

CodeLattice 当前不是要再做一个“大而全”的代码扫描器，也不是临时修几个 MCP mode。

当前优化方向是：

> **把 CodeLattice 调整成 AI 使用友好的本地代码上下文压缩器：快、并发、可靠、省 token，并且优先反映当前工作区。**

对 AI 来说，好用不是“返回全量图谱”，而是：

- 马上知道该看哪里；
- 马上知道当前改动有没有影响；
- 马上知道结果是新的、旧的、部分的，还是后台正在补；
- 拿到足够证据，不需要重新 `rg` 一遍；
- 不被大项目全量分析卡住；
- 不因为多个查询/项目并发而互相堵死。

---

## 2. 用户目标

用户明确要解决的是当下 AI 使用体验的底座问题：

1. **性能**  
   AI 第一次问项目、符号、调用链、影响分析时，不应该等几十秒到几分钟。

2. **并发**  
   backend / frontend、project / symbol / impact 等分析和查询不能互相卡死。控制面调用必须永远快。

3. **可靠**  
   结果不能误导 AI。静态分析不是 runtime proof；旧索引不是当前真相；局部结果必须标注边界。

4. **省 token**  
   CodeLattice 应该替 AI 压缩上下文，而不是把大 JSON 塞给 AI。默认输出应是 AI 决策卡 + 最小证据 + detail handle。

5. **当前工作区优先**  
   GitNexus 的体验问题之一是偏提交级索引：索引 stale 时 AI 容易基于旧世界回答。CodeLattice 要更适合编程现场：**新改动优先，旧索引兜底，全量刷新后台跑。**

---

## 3. 当前 CodeLattice 已经做到的事

截至本轮讨论和代码检查，CodeLattice 已经具备这些基础：

- 默认 AI MCP toolset 已收敛为 6 个 facade 工具，full toolset 保持 49。
- 默认持久缓存路径已经设计为 `~/.cache/codelattice`，用户不必手动配置 `CODELATTICE_CACHE_DIR` 才能用。
- 大项目 cache miss/stale 时已有自动 job 化路径，会返回 `analyzing + jobId`，避免同步阻塞。
- `job_status` / `job_detail` / `job_cancel` 属于控制面，应该绕过 busy。
- `warmTrace` 已经证明 facade `GraphView` / JSON 不是主要瓶颈。
- open-nwe/backend 度量显示 `rustAnalysisMs` 占 warm time 绝大多数，因此后续性能优化应关注 Rust analyzer pipeline。
- 已经开始补 `analysisTrace`，尝试拆细 `rustAnalysisMs`。

这些都是好基础，但还不是最终 AI Query Runtime。

---

## 4. 当前主要缺口

### 4.1 stale cache 现在更像 miss，而不是 baseline

当前倾向是：

```text
cache stale
  -> invalidate / remove
  -> miss
  -> submit job
  -> AI 等 job 完成后重试
```

这对“绝对新鲜”是保守的，但对 AI 使用不顺手。AI 在最需要上下文的时候失去旧索引，只能等后台 job 或自己 `rg`。

目标应改成：

```text
cache stale
  -> 保留为 stale baseline
  -> 识别 changed files
  -> 快速分析 working-tree delta
  -> 返回 fresh delta + stale baseline
  -> 后台 refresh 完整 snapshot
```

### 4.2 没有 working tree delta 层

当前没有明确的“只分析当前改动文件并覆盖旧索引”的查询层。

AI 更需要的是：

```text
fresh delta > current snapshot > stale baseline
```

用户刚改过的文件必须第一时间被看到。旧索引用来补全全局背景，而不是覆盖新代码。

### 4.3 queued 需要确认是真队列

当前代码里有 queued response，但执行 AI 必须确认：

- queued job 是否真的会被调度执行；
- 是否只是返回一个 queued handle；
- 达到并发上限后，排队任务是否会在 active job 结束后自动启动；
- 如果没有真队列，应该返回 honest backpressure，不能让 AI 以为已经排队。

### 4.4 查询和分析还没有统一 Query Planner

现在各 facade mode 分散处理：

- project quick
- symbol search
- call_chains
- change_review impact
- workflow ask/diagnose

它们不应该各自决定是否全量分析、是否 job、是否 compact。需要一个统一 Query Planner 判断：

- 有没有 fresh delta；
- 有没有 usable snapshot；
- snapshot 是否 stale；
- 是否能 partial answer；
- 是否需要 background refresh；
- 输出应该省略哪些大 payload；
- 应该给 AI 哪些 next calls。

### 4.5 token-aware output 还不是硬契约

compact 已经做了一些收敛，但还没有统一成“AI 决策卡”契约。默认输出应该避免：

- 重复 rootDiagnosis/sourceOnlyEntries；
- 大量原始 edges/nodes；
- 没有排序价值的长列表；
- 只告诉 count 不给证据；
- 只给风险等级不给文件/行号/edge reason。

---

## 5. 目标运行模型

CodeLattice 应该采用如下运行模型：

```text
AI Query
  -> Query Planner
      -> Working Tree Delta Store
      -> Snapshot Store
          -> fresh snapshot
          -> stale baseline snapshot
          -> building snapshot
      -> Facade Query Index
      -> Job Scheduler
  -> AI Decision Card
```

### 5.1 Snapshot Store

Snapshot Store 保存可查询快照，而不是只有“命中/未命中”：

- `fresh`: 与当前工作区一致；
- `stale`: 旧但可用，必须标注 stale reason；
- `building`: 后台正在刷新；
- `missing`: 没有可用快照。

stale snapshot 不应默认删除。它是全局背景，价值很高。

### 5.2 Working Tree Delta

当文件变化时，先生成 delta：

- `changedFiles`;
- 新增/修改/删除的 symbols；
- 改动文件内 direct calls/imports；
- 能确认的局部 callers/callees；
- 与 baseline 的冲突/缺口。

Delta 的优先级高于 baseline：

```text
working tree delta > fresh snapshot > stale baseline
```

### 5.3 Facade Query Index

不要每次从完整图现场算 AI 常用查询。持久化或内存维护以下索引：

- `symbolName -> symbolIds`;
- `symbolId -> source location`;
- `symbolId -> callers`;
- `symbolId -> callees`;
- `symbolId -> impact adjacency`;
- `file -> symbols`;
- `route -> handler`;
- `module -> risk/readFirst`;
- `component -> files/symbols/risk`;
- `changedFile -> affectedSymbols`;

这些索引是 AI 使用体验的核心。查询应优先走索引，完整图是底层材料，不是默认输出。

### 5.4 Job Scheduler

Job Scheduler 必须成为真实底座，不只是避免 busy：

- control-plane 永远快；
- query 类优先；
- background refresh 次优先；
- same root/language/mode singleflight；
- different root 可以并行；
- 有全局并发上限，避免打满机器；
- queued 必须真会执行；
- cancel 必须能阻止后续 summary 覆盖；
- job_detail 在 running 时可以返回 partial progress，而不是完全不可用。

### 5.5 Token-Aware Output

所有 facade 默认返回 AI 决策卡：

```json
{
  "answer": "改 helper 主要影响 main_fn 和 api_handler，风险 medium。",
  "freshness": {
    "mode": "fresh_delta_plus_stale_baseline",
    "freshFiles": ["src/lib.rs"],
    "baselineStale": true,
    "staleReason": "file_modified"
  },
  "evidence": [
    {
      "file": "src/main.rs",
      "line": 12,
      "symbolId": "symbol:...",
      "edge": "main_fn -> helper",
      "reason": "static CALLS edge"
    }
  ],
  "confidence": {
    "level": "medium",
    "missingEvidence": ["dynamic dispatch not resolved", "background refresh running"]
  },
  "omitted": {
    "callerCount": 128,
    "edgeCount": 483,
    "detailAvailableVia": "job_detail(jobId=..., page=0, pageSize=50)"
  },
  "tokenBudget": {
    "estimatedResponseTokens": 1200,
    "omittedItemCount": 432,
    "savedByOmittingDetails": true
  },
  "nextActions": []
}
```

默认不是“少给”，而是“给 AI 决策所需的最小充分证据”。

---

## 6. 可靠性原则

CodeLattice 不能为了快而瞎说。每个结果都要说明来源：

| 来源 | 含义 | AI 应如何使用 |
|------|------|---------------|
| `fresh_delta` | 当前工作区改动文件的局部分析 | 最高优先级，但覆盖范围有限 |
| `fresh_snapshot` | 与当前工作区一致的完整索引 | 可作为主要依据 |
| `stale_baseline` | 旧索引，全局背景 | 可用于背景，不可当作当前真相 |
| `partial` | 后台分析未完成的部分结果 | 可作为候选，需要后续确认 |
| `missing` | 没有足够证据 | 必须告知 AI 不要强断言 |

输出必须保留：

- 文件路径；
- 行号；
- symbol id；
- edge reason；
- confidence/reason；
- static-only caveat；
- freshness metadata。

否则 AI 还是会不放心，重新 `rg` 和读文件。

---

## 7. 不要偏航的方向

当前不是做这些：

- 不是继续暴露更多底层 MCP tools；
- 不是把 `ask` 做成聊天机器人；
- 不是返回更大的 graph JSON；
- 不是追求所有语言同等深度；
- 不是把所有 cache stale 都当 miss；
- 不是只做 rayon 并行就算完成；
- 不是为了 compact 把证据删掉；
- 不是假 queued；
- 不是把静态分析包装成 runtime/coverage proof。

Rust analyzer 并行化很重要，但它只是性能底层的一部分。AI 使用体验还需要 stale baseline、working tree delta、query planner、token-aware output 一起成立。

---

## 8. 当前执行优先级

### P0 — 先恢复可验证状态

如果工作区有未提交改动，先确认：

- `cargo check` 是否通过；
- `run_rust_analysis` 返回签名是否只影响 Rust 路径；
- TypeScript/JavaScript/Python 适配器不要被误改成 Rust 的 4 元组返回；
- `analysisTrace` 字段进入 job summary 且不破坏现有 schema。

### P1 — 明确 Freshness / Snapshot Contract

先设计并测试这些状态，不急着重构所有实现：

- `fresh_snapshot`;
- `stale_baseline`;
- `fresh_delta`;
- `fresh_delta_plus_stale_baseline`;
- `background_refresh_running`;
- `partial_result`;
- `missing_evidence`;
- `tokenBudget`.

### P2 — stale cache 不再删除，改为 usable baseline

`try_load_persistent` / memory cache stale 检查应能返回 stale entry，而不是只返回 `None`。

需要把 cache lookup 从二态：

```text
hit | miss
```

升级为：

```text
fresh_hit | stale_hit | miss | corrupted
```

### P3 — working tree delta 快速路径

先支持最关键场景：

- modified Rust source file；
- symbol search 能看到新/改名 symbol；
- impact 能标注 old baseline + fresh delta；
- 删除 symbol 时能说明 baseline 中存在但 current delta 缺失。

### P4 — 真队列和并发

确认并修正：

- queued job 必须被 worker 消费；
- active job 完成后 queued job 自动启动；
- control-plane 不受影响；
- same root/language singleflight；
- different root 并发；
- 并发上限可配置，默认保守。

### P5 — Rust analyzer pipeline 并行化

在 `analysisTrace` 证明具体瓶颈后再做：

- symbol extraction file-level parallel；
- call resolution file-level parallel；
- deterministic ordering tests；
- open-nwe read-only before/after。

---

## 9. 验收标准

一个 AI-friendly foundation pack 不能只说“测试通过”，还必须回答这些问题：

1. **快吗？**
   - cold miss 是否立即返回 job/partial；
   - fresh/stale hit 是否秒回；
   - `rustAnalysisMs` 是否有 before/after；
   - control-plane 是否稳定 < 1s。

2. **并发可靠吗？**
   - 同 root 是否 singleflight；
   - 不同 root 是否可并发；
   - queued 是否真执行；
   - cancel 是否有效；
   - job_detail running 是否能提供 progress/partial。

3. **新代码优先吗？**
   - modified file 的新 symbol 能否立即被 search 找到；
   - impact 是否区分 fresh delta 和 stale baseline；
   - stale baseline 是否不误导为 fresh。

4. **省 token 吗？**
   - compact 输出是否是 AI 决策卡；
   - 是否有 evidence 而非大 payload；
   - 是否有 omitted/detail handle；
   - 是否减少 rootDiagnosis/sourceOnlyEntries 重复。

5. **AI 还需要 rg 吗？**
   - 对 symbol search / callers / impact / diagnose，默认输出是否已经给出文件、行号、证据、置信度；
   - 如果 AI 仍需要 `rg`，是因为 missing evidence 被明确标注，还是工具没给够信息？

---

## 10. 给执行 AI 的开场提示

如果你是接手执行的 AI，请先读本文件，然后执行以下判断：

```text
1. 当前工作区是否干净？
2. 如果不干净，这些改动是谁的、是否能编译？
3. 当前是否已经有 analysisTrace？
4. 当前 queued 是否真队列？
5. 当前 persistent stale 是否会被删除？
6. 当前 symbol/search/impact 是否能基于 stale baseline 回答？
7. 当前输出是否有 freshness + evidence + tokenBudget？
```

不要直接开始写功能。先把当前运行时状态画清楚，再改。

---

## 11. 最终目标

CodeLattice 的终局不是“知道一切”，而是：

> **最快给 AI 足够可靠的下一步。**

具体表现：

```text
当前改动：马上局部分析，保证新
旧索引：马上补背景，保证快
后台 job：并行刷新，保证完整
查询输出：压缩成 AI 决策卡，保证省 token
证据标注：文件/行号/置信度/freshness，保证靠谱
```

这就是当前优化方向。后续加语言、加诊断、加功能，都必须服务于这个底座，而不是绕开它。
