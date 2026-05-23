# Analysis Engine Parallelism Roadmap

Date: 2026-05-23

## Purpose

CodeLattice 当前已经具备多语言静态图谱分析、workspace 发现、跨项目图谱、MCP facade 和 WebUI workbench。MCP 并发稳定性已经通过 `mcp_server_busy` 守住了 1.0 的可用性：AI 客户端并行调用不会再打断 MCP session。

下一阶段的核心不是继续修 MCP stdio，而是建设 **Analysis Engine 1.2/1.3**：

> 面向大项目和大 workspace，把 discovery / parse / symbol / import / reference / calls / graph merge 拆成可并行、可缓存、可增量、可解释的分析任务，并由统一 scheduler 管理。

这份路线图用于指导后续执行 AI 分包实现，避免直接在 `mcp_server.rs` 或各语言 adapter 中临时加线程。

## Current State

### Already Solved

- MCP 默认对 AI 暴露 6 个 facade entry tools。
- MCP 并发调用不会再导致 stdio session 断连；重叠调用返回 `codelattice.mcpBusy.v1`。
- Workspace root 可以作为自然入口，支持大根目录 auto-entry。
- `analysis-scheduler` 已经具备初步 phase / fingerprint / incremental plan 元数据。
- 多语言 adapter 已覆盖 Rust、Cangjie、ArkTS、TypeScript、JavaScript、C、C++、Python、Shell。

### Remaining Gap

当前分析路径整体仍偏“请求内同步执行”：

- 大项目上的 parse/index/graph 构建不能充分并行。
- 不同工具可能重复做相同 root 的扫描或图构建。
- 中间产物复用粒度不够细。
- 长任务缺少统一 job/progress/cancel/paged result 语义。
- 语言 adapter 的并发边界没有统一 contract。

## Version Target

### 1.1: MCP Job Runtime

目标：AI 可以并发发起请求，server 稳定、可排队、可查询、可取消。

范围：

- job id
- job status
- job dedupe
- worker limit
- cancel
- compact result
- paged detail
- MCP facade 保持 6 个入口

1.1 主要解决 “AI 用得顺手”，但不要求分析内核真正并发化。

### 1.2: Parallel Analysis Kernel

目标：单项目内部的文件级分析可并行，且输出与串行版本确定性一致。

范围：

- Analysis DAG contract
- deterministic graph reducer
- file-level parse/symbol/import/reference worker pool
- per-file failure isolation
- progress events
- Rust / TypeScript / JavaScript / Python 试点

### 1.3: Large Workspace Production Runtime

目标：大 workspace 多项目并发、增量 cache 和 paged graph result 成熟可用。

范围：

- 全语言 adapter 接入统一 contract
- workspace 多项目 worker scheduling
- shared intermediate artifact store
- incremental graph rebuild
- cancel / resume
- real corpus benchmark
- MCP/WebUI job runtime 集成

## Architecture Direction

```text
AI client / CLI / WebUI
  ↓
Facade request router
  ↓
Analysis job runtime
  ↓
Analysis DAG planner
  ↓
Worker pool
  ├─ discover tasks
  ├─ parse tasks
  ├─ symbol tasks
  ├─ import/reference tasks
  └─ calls tasks
  ↓
Deterministic graph reducer
  ↓
Artifact cache / graph snapshot store
  ↓
Compact result / paged detail / report
```

## Core Principles

1. **Concurrency belongs in the analysis engine, not in every language adapter.**
   Language adapters should expose deterministic functions and capability metadata. They should not spawn their own background threads.

2. **Workers produce immutable intermediate artifacts.**
   Shared mutable graph writes from many workers are forbidden. Workers emit file/module/project artifacts; reducers merge them.

3. **Graph merge must be deterministic.**
   Given the same inputs, output node IDs, edge IDs, ordering, confidence, and diagnostics must be stable regardless of worker order.

4. **Cache must be content-addressed.**
   Cache keys should include file path, content hash, language, adapter version, parser version, and relevant options.

5. **Partial failure should be explicit.**
   A failed file task should produce diagnostics and mark quality/coverage limits; it should not crash the whole analysis unless the root itself is invalid.

6. **Static-only semantics remain unchanged.**
   No target project code execution, no build script execution, no runtime proof, no compiler/coverage proof.

## Proposed Packs

### Pack 1: Analysis DAG Contract Pack

Goal: Make the analysis flow explicit without changing behavior.

Deliverables:

- `analysis-engine` or expanded `analysis-scheduler` crate.
- `AnalysisPipeline`, `AnalysisStage`, `AnalysisTask`, `AnalysisArtifact`, `AnalysisPlan`.
- Serial executor that reproduces current behavior.
- Stage timing and artifact count metadata.
- Compatibility adapter so existing CLI/MCP output remains unchanged.

Acceptance:

- Existing tests pass.
- Portable fixtures produce equivalent output before/after.
- A debug/JSON plan can explain stages, dependencies, and produced artifacts.

Non-goal:

- No real parallel execution yet.
- No language-wide refactor yet.

### Pack 2: Deterministic Graph Merge Pack

Goal: Establish a reducer that can merge unordered artifacts into stable graph output.

Deliverables:

- Stable node ID and edge ID policy.
- Dedupe policy.
- Deterministic sort policy.
- Dangling edge guard.
- Confidence/reason preservation.
- Merge diagnostics.
- Snapshot tests with shuffled input artifacts.

Acceptance:

- Shuffled worker outputs produce byte-stable graph JSON after normalization.
- Dangling CALLS edges remain zero.
- Existing graph contract tests pass.

Non-goal:

- No new call-resolution strategy.
- No behavior-changing confidence upgrades.

### Pack 3: Parallel File Analysis Pack

Goal: Add bounded worker pool for file-level parse and extraction.

Deliverables:

- Worker pool with configurable max concurrency.
- File task scheduling.
- Per-file timeout and panic isolation.
- Per-file parse/symbol/import/reference artifacts.
- Progress events.
- Serial-vs-parallel parity tests.

Acceptance:

- Parallel output matches serial output for selected fixtures.
- Single file failure is isolated and visible as diagnostics.
- Memory and worker count are bounded.
- At least one large fixture or corpus run shows measurable speedup.

Initial languages:

- Rust
- TypeScript
- JavaScript
- Python

Non-goal:

- Do not parallelize graph merge by mutating shared graph state.
- Do not rewrite all language adapters in one pack.

### Pack 4: Shared Intermediate Cache Pack

Goal: Reuse parse/symbol/import/reference artifacts across tools and runs.

Deliverables:

- Content-addressed artifact keys.
- Artifact store for file-level and project-level intermediates.
- Cache hit/miss/stale reasons.
- Per-stage reuse metadata.
- Incremental plan based on changed files and dependency fanout.
- `cacheSemantics` extension for intermediate artifacts.

Acceptance:

- First run populates artifacts.
- Second run reuses most artifacts.
- Editing one file only invalidates relevant file tasks plus dependent project merge stages.
- MCP/CLI can explain reused vs rerun stages.

Non-goal:

- No unsafe reuse when adapter version/parser version/options changed.

### Pack 5: Language Adapter Concurrency Contract Pack

Goal: Make language adapters implement a consistent interface for the engine.

Suggested adapter contract:

```text
discover_units(root, options) -> AnalysisUnit[]
parse_unit(unit, source) -> ParseArtifact
extract_symbols(parse_artifact) -> SymbolArtifact
extract_imports(parse_artifact) -> ImportArtifact
extract_references(parse_artifact) -> ReferenceArtifact
adapter_capabilities() -> AdapterCapabilities
```

Rollout order:

1. Rust
2. TypeScript / JavaScript
3. Python
4. C / C++
5. Shell
6. ArkTS / Cangjie

Acceptance:

- Each adapter declares supported stages.
- Adapter output is serializable and cacheable.
- Adapter does not spawn its own unmanaged threads.
- Unsupported or partial capability is explicit.

Non-goal:

- Do not force every language to support every stage before the engine ships.

### Pack 6: MCP Job + Large Workspace Runtime Pack

Goal: Connect the concurrent engine back to AI workflows.

Deliverables:

- Job submission and status for heavy analysis.
- `queued` / `running` / `succeeded` / `failed` / `cancelled` lifecycle.
- Progress events: stage, completed units, total units, current project.
- Job dedupe for same root/language/options.
- Compact result by default.
- Paged detail for graph nodes/edges/symbol matches.
- Cancellation hooks.
- Same 6 facade tools; no tool surface explosion.

Acceptance:

- AI can issue several large requests without MCP disconnect.
- Duplicate same-root requests reuse running jobs or cached artifacts.
- Large workspace requests return status quickly.
- `open-nwe` class workspace can be explored through progress/result pages instead of blocking the client.

Non-goal:

- No WebUI redesign required in this pack; WebUI can consume the same job API later.

## Data Model Sketch

### AnalysisTask

```json
{
  "id": "task:file:src/lib.rs:parse",
  "stage": "parse",
  "root": "/repo",
  "language": "rust",
  "unitId": "file:src/lib.rs",
  "dependsOn": ["task:file:src/lib.rs:fingerprint"],
  "cacheKey": "sha256:..."
}
```

### AnalysisArtifact

```json
{
  "schemaVersion": "codelattice.analysisArtifact.v1",
  "artifactKind": "symbols",
  "language": "rust",
  "unitId": "file:src/lib.rs",
  "cacheKey": "sha256:...",
  "generatedFrom": {
    "staticAnalysis": true,
    "runtimeVerified": false,
    "targetCodeExecuted": false
  }
}
```

### AnalysisJob

```json
{
  "schemaVersion": "codelattice.analysisJob.v1",
  "jobId": "job_...",
  "status": "running",
  "root": "/repo",
  "language": "auto",
  "stage": "symbols",
  "progress": {
    "completedUnits": 120,
    "totalUnits": 600
  }
}
```

## AI Execution Guidance

When handing this roadmap to an execution AI, do not ask for “full true concurrency” in one task. Use these prompts in order:

1. “Implement Analysis DAG Contract Pack. Preserve behavior. No parallel execution yet.”
2. “Implement Deterministic Graph Merge Pack. Prove shuffled artifacts produce stable output.”
3. “Implement Parallel File Analysis Pack for Rust + TypeScript/JavaScript + Python only.”
4. “Implement Shared Intermediate Cache Pack for parse/symbol/import/reference artifacts.”
5. “Implement Language Adapter Concurrency Contract rollout for remaining adapters.”
6. “Implement MCP Job Runtime backed by the parallel analysis engine.”

Each pack should include:

- preflight doc
- execution card
- fixture or corpus validation
- serial-vs-new parity check
- native precommit
- closure doc

## Risk Register

| Risk | Why It Matters | Mitigation |
|---|---|---|
| Non-deterministic graph output | Breaks tests, cache, AI trust | Stable reducer and shuffled-input tests |
| Cache unsafely reuses stale artifacts | Incorrect analysis | Content-addressed keys include adapter/parser/options |
| Worker pool overuses memory | Large projects become unstable | Bounded concurrency and artifact size accounting |
| Adapter refactor changes semantics | Language regressions | Per-language parity fixtures |
| MCP returns huge results | AI context blowup | Compact defaults and paged detail |
| Partial failures hidden | False confidence | Explicit diagnostics and generatedFrom limitations |

## Success Metrics

### 1.2 Metrics

- Parallel and serial outputs match on selected fixtures.
- File-level worker pool handles at least 1,000 source files without session failure.
- Cache explain can show per-stage reuse.
- At least Rust, TypeScript, JavaScript, and Python are engine-backed.

### 1.3 Metrics

- Large workspace analysis supports multi-project scheduling.
- Re-analyzing after a one-file change avoids full parse/index rebuild.
- MCP job API can report progress and return paged results.
- AI clients can explore large repos without parallel-call crashes or context blowups.

## Stop Lines

- Do not execute target project code.
- Do not run build scripts, proc macros, package scripts, or user tests as part of static analysis.
- Do not make language adapters spawn unmanaged threads.
- Do not merge graph output from worker threads through shared mutable graph writes.
- Do not introduce non-deterministic node/edge ordering.
- Do not expand MCP default tool surface beyond the 6 facade entry tools for this work.

## Recommended Next Step

Start with **Analysis DAG Contract Pack + Deterministic Graph Merge Pack** as the foundation. This gives later execution AI a safe substrate for real parallelism without immediately touching every language adapter or MCP job runtime.
