# Incremental Dirty-file Planning Design

Date: 2026-05-20
Status: Approved by user direction

## Goal

Give CodeLattice a scheduler-level dirty-file plan that explains why a stale cache entry needs fresh analysis and which phases are affected.

## Scope

This pack is a planning foundation. It does not implement partial graph rebuild execution. MCP still runs the existing full analysis command on cache miss, and the returned plan explicitly says it is `planOnly`.

## Architecture

The scheduler crate will expose a compact file snapshot for every scheduler-tracked file. A cache entry stores that snapshot. On the next lookup, MCP passes the previous snapshot into `build_schedule`; the scheduler compares old and current snapshots and emits an `incrementalPlan`.

```text
cached file snapshot
  -> build_schedule(previous_files)
  -> compare current vs previous
  -> dirty files + affected phases
  -> MCP metadata
  -> existing full analyze path
```

## Dirty File Model

Each dirty file carries:

- relative path
- status: `added`, `removed`, or `modified`
- extension
- reason
- affected phases

The plan caps the visible dirty-file list so large repos do not flood MCP responses. Summary counts remain complete.

## Execution Strategy

- `reuse`: no dirty files and fingerprint match.
- `fullAnalysis`: manifest/config/removal or mixed changes require the current full path.
- `fileScopedCandidate`: source-only modifications can be identified as future candidates for partial graph rebuild, but still run full analysis in this pack.

## Testing

- Scheduler unit test: mutate a `.rs` file and assert `incrementalPlan` reports one modified file, source phases, and `planOnly`.
- MCP test: populate cache, mutate `config/schema.yaml`, assert cache miss metadata includes `incrementalPlan` with the YAML file and full-analysis strategy.

