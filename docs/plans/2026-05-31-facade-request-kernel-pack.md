# Facade Request Kernel Pack

> Execution card for the next CodeLattice MCP architecture pass.

## Goal

Make the six AI facade tools feel like one coherent query runtime: a user or AI can pass a workspace/project/focused root, `language=auto`, and `compact=true`, and every facade path should resolve root/language/cache/job/freshness/output rules consistently.

## Current State

- Performance and queueing have improved: analysis jobs are non-blocking, queued, cancellable, and can warm the facade cache.
- Workspace-root auto-routing exists for `codelattice_symbol`, `codelattice_change_review`, `codelattice_workflow(before_edit)`, and `codelattice_workflow(ask)`, but the logic is still duplicated around the individual handlers.
- Compact output has been optimized, but the token contract is still enforced in handler-specific branches.
- Language runtime traces exist for Rust and job warm paths, but there is no unified adapter capability contract across Rust/TypeScript/Python/C/C++/Cangjie.
- `crates/cli/src/mcp_server.rs` is ~28K lines and now contains transport, request normalization, root routing, facade orchestration, output shaping, cache policy, and job policy.

## Architecture Direction

### P0: Facade Request Kernel

Introduce a single internal request context for facade tools:

- `originalRoot`, `effectiveRoot`
- `requestedLanguage`, `effectiveLanguage`
- `mode`, `tool`, `compact`
- workspace root routing decision and confidence
- canonical root/language used for cache keys
- normalized params passed to lower-level handlers

Handlers should ask the kernel once and then run against `effectiveRoot/effectiveLanguage`. This removes per-tool disagreement around `language=auto`, workspace roots, and follow-up arguments.

### P1: Facade Module Boundary

Do not start with a risky full file split. Pack A creates only a narrow `mcp_facade.rs` helper module for request/response contracts, then later packs can move full handlers once tests pin behavior.

Target module boundaries:

- `facade/request.rs`: request context, root routing, normalized params
- `facade/response.rs`: compact/token envelope, context refs, response annotation
- `facade/project.rs`, `facade/symbol.rs`, `facade/change_review.rs`, `facade/workflow.rs`, `facade/ask.rs`

### P2: Central Compact/Token Contract

All facade responses should share these compact guarantees:

- bounded output, target under 16KB for compact smoke cases
- no repeated full `sourceOnlyEntries`, `schedule.phases`, or verbose static semantics
- top-N evidence with explicit `omitted`
- direct-copy `recommendedNextCalls`
- an explicit request context so AI does not need to infer which root/language was used

### P3: Language Runtime Capability Contract

Add a compact per-language runtime capability object for analysis/job outputs:

- `language`
- `inProcessAnalysis`
- `cliFallbackUsed`
- `supportsDeltaOverlay`
- `supportsCallEdges`
- `supportsPersistentCache`
- `traceAvailable`

This does not optimize every language in this pack. It creates the contract so future TS/Python/C/C++ work targets the same evidence surface instead of one-off behavior.

## Write Set

- `docs/plans/2026-05-31-facade-request-kernel-pack.md`
- `crates/cli/src/mcp_facade.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`

## Forbidden Set

- Do not modify live repositories such as `open-nwe`, `cangjie`, `warp`, or `openfang`.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` unless explicitly requested.
- Do not change MCP tool counts: default AI toolset remains 6, full toolset remains 49.
- Do not change graph schema semantics or introduce dangling `CALLS` edges.
- Do not rewrite TypeScript/Python/C/C++ analysis in this pack.

## Stop Lines

- Stop and report if toolset count changes.
- Stop and report if compact output grows beyond 16KB in existing compact smoke tests.
- Stop and report if workspace-root auto-routing regresses for symbol/change_review/workflow/ask.
- Stop and report if precommit reports high/critical risk that is not explained by this facade refactor.

## Pack A Tasks

1. Add `FacadeRequestContext` and the first `mcp_facade.rs` helper boundary.
2. Rewire `codelattice_symbol` and `codelattice_change_review` to use the context.
3. Add request-context annotation to wrapped facade output.
4. Add language runtime capability metadata for facade responses.
5. Add tests covering:
   - `requestContext` exists on compact symbol and change_review responses.
   - workspace routed responses expose original/effective root/language consistently.
   - `language=auto` resolves into `effectiveLanguage`.
   - compact output still strips repeated schedule/source-only data.
   - toolset remains 6/49.

## Pack B Tasks

1. Extend the shared facade contract to `codelattice_project`, `codelattice_workspace`, and auto-job responses.
2. Ensure compact project decision cards keep `requestContext`, `runtimeCapabilities`, `omitted`, and `tokenBudget`.
3. Ensure large-project auto-job responses expose the same root/language contract as synchronous facade responses, so AI can poll the right job and retry the warmed root without guessing.
4. Add a small central response helper in `mcp_facade.rs` for attaching request context, runtime capability metadata, and compact token metadata.
5. Add tests covering:
   - project quick compact decision cards expose the normalized request context.
   - project auto-job `analyzing` responses expose the normalized request context.
   - workspace compact responses expose the normalized request context.

## Pack C Tasks

1. Extend the same facade request contract to explicit job control-plane paths:
   `mode=job`, `mode=job_status`, `mode=job_detail`, and `mode=job_cancel`.
2. Keep root-less polling ergonomic: when `job_status` / `job_detail` omit `root`
   and `language`, infer `effectiveRoot` and `effectiveLanguage` from the job
   registry.
3. Keep job responses copy-pasteable for AI agents by attaching
   `requestContext`, `runtimeCapabilities`, compact `omitted`, and `tokenBudget`
   metadata to both the busy-guard control-plane fast path and the facade job
   handlers.
4. Add tests covering:
   - explicit `codelattice_project(mode=job)` responses expose
     `requestContext` and the resolved `effectiveLanguage`.
   - `job_status` and `job_detail` infer context from `jobId` without requiring
     the caller to resend `root`.

## Verification

Minimum:

```bash
cargo fmt --check
git diff --check
cargo test --test mcp_server
scripts/codelattice-installed-acceptance.sh --dev-only
scripts/codelattice-precommit-check.sh
```

Preferred before push:

```bash
cargo test
```
