# Scheduler Cache Reuse Design

Date: 2026-05-20
Status: Approved by user direction

## Goal

Make CodeLattice cache reuse depend on the scheduler fingerprint, not only on the older source mtime map.

## Scope

This pack updates cache freshness behavior only. It does not add a parser, interpreter, daemon, watcher, parallel scheduler, or new language semantics.

## Architecture

The scheduler crate remains the owner of request normalization, filesystem fingerprinting, and cache decision labels. The MCP cache layer stores the scheduler fingerprint alongside existing metadata and asks the scheduler whether a cached entry is reusable before returning it.

```text
MCP cache lookup
  -> cached scheduler fingerprint
  -> build current schedule(previous_fingerprint)
  -> reuse only when decision.action == reuse
  -> otherwise invalidate and run fresh analysis
```

## Behavior

- Memory cache returns a hit only when the current scheduler fingerprint matches the cached fingerprint.
- Persistent cache returns a hit only when the stored scheduler fingerprint matches the current scheduler fingerprint.
- Existing mtime, manifest, and docs checks remain in place as conservative compatibility checks.
- A non-source file that is tracked by scheduler fingerprinting, such as a project-local `.yaml`, invalidates cache even though the old source-extension mtime map ignored it.
- Hidden/generated directories remain ignored by the scheduler fingerprint.

## Testing

Two MCP tests define the new behavior:

- Memory cache: analyze a temp Rust fixture, mutate `config/schema.yaml`, and assert the next analyze is a miss with scheduler stale metadata.
- Persistent cache: populate cache in one session, verify a persistent hit in another, mutate `config/schema.yaml`, and assert the next process misses.

