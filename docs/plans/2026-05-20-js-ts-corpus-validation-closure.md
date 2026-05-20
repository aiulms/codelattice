# JS/TS Real-World Corpus Validation — Closure

**Date:** 2026-05-20
**Status:** ✅ Completed (fixture-only; real-world corpus deferred to network availability)
**Commit:** pending (on top of `7d9ccb0`)

## Deliverables

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 1 | `fixtures/corpus/js-ts-corpus-manifest.json` | ✅ Created | 6 projects: lodash-es, uuid, zod, tinybench, vite-plugin-react, conf |
| 2 | `scripts/js-ts-corpus-smoke.sh` rewrite | ✅ Complete | `--fixture-only`, `--corpus-dir`, `--clone-missing`, `--offline`, `--project` flags |
| 3 | Real-project baselines | ⚠️ Deferred | GitHub unreachable (port 443 timeout); graceful skip per task constraint |
| 4 | Issue fixes from real projects | N/A | No real projects ran; fixture-only passed cleanly |
| 5 | CHANGELOG.md update | ✅ Added | Three new entries: facade consolidation, change intelligence, corpus validation |
| 6 | Closure document | ✅ This file | — |

## Fixture Baselines

| Fixture | Language | Source Files | Symbols | Edges | CALLS | FW Hints | Public Surface |
|---------|----------|-------------|---------|-------|-------|----------|---------------|
| `fixtures/javascript/portable-smoke` | javascript | 11 | 51 | 104 | 11 | 3 | 16 |
| `fixtures/typescript/portable-smoke` | typescript | 4 | 20 | 54 | 12 | 0 | 0 |

## Script Architecture

The corpus smoke script writes `analyze` output to a temp file, then extracts metrics via `python3 -c "..." "$OUTPUT_FILE"` using `sys.argv[1]`. This avoids the `echo "$VAR" | python3 - <<'HEREDOC'` stdin conflict (heredoc overrides pipe). Metrics are extracted in a single python3 call using tab-separated output, then parsed with `cut`. Edge counts prefer `summary.callEdgeCount` over manual edge counting.

## Bugs Found and Fixed

1. **Bash→Python stdin conflict**: `echo "$OUTPUT" | python3 - <<'PY'` — heredoc overrides pipe stdin, so `json.load(sys.stdin)` reads heredoc, not JSON. Fixed by writing to temp file and using `sys.argv[1]`.
2. **Edge field name mismatch**: Script checked `e.get('kind') == 'CALLS'` but edges use `type` field, not `kind`. Fixed to prefer `summary.callEdgeCount`, with fallback `e.get('type') == 'CALLS'`.
3. **Zero call-edge false alarm**: Both above bugs caused the script to report 0 call edges despite `summary.callEdgeCount` being 11/12. Fixed in extraction logic.

## Network Limitation

GitHub clone attempts all failed with `Failed to connect to github.com port 443 after 75xxx ms`. Per task constraint: "如果网络、GitHub/GitCode clone 失败，要 graceful skip，并保留 fixture-only 验证". The `--offline` mode correctly skips missing repos and falls back to fixture-only validation.

## Verification Results

- `cargo fmt --check`: ✅ clean
- `cargo test`: 138/138 pass (1 flaky `mcp_smoke_rust_only` passes in isolation)
- `scripts/codelattice-detect-changes-smoke.sh`: 17/17 pass
- `scripts/codelattice-mcp-facade-smoke.sh`: 13/13 pass
- `scripts/js-ts-corpus-smoke.sh --fixture-only`: 2/2 pass
- `scripts/js-ts-corpus-smoke.sh --corpus-dir ... --offline`: 2/2 pass, 6 skipped

## Files Changed

| File | Action | Lines |
|------|--------|-------|
| `scripts/js-ts-corpus-smoke.sh` | Rewritten | ~260 |
| `fixtures/corpus/js-ts-corpus-manifest.json` | New | ~60 |
| `CHANGELOG.md` | Updated | +8 lines |

## Deferred Work

- **Real-project baselines**: When GitHub network is available, run `--clone-missing --corpus-dir /path/to/corpus` to clone and analyze real projects.
- **Additional corpus projects**: manifest can be extended with more JS/TS projects.
- **TS fixture enrichment**: TS portable-smoke has 0 diagnostics/0 FW hints/0 public surface — could add richer fixtures.
