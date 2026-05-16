# Public API / External Consumer Caution Pack — Preflight

**Date:** 2026-05-16  
**Status:** Preflight  
**Depends on:** b2a5603 (reachability map), d01e631 (diagnostics pack)

## 1. Problem

`dead_code_candidates` / `reachability_map` can find "unreachable within repo" symbols.
But the most dangerous false-positive is: **"no one in-repo calls it, but external consumers do."**

Examples: Rust `pub` API, TS package exports, Python `__init__.py` re-exports, C/C++ headers, Cangjie public symbols, ArkTS exported components.

## 2. Goal (NOT a proof)

Add a **unified external API surface detection layer** that:
- Identifies symbols/files **likely consumed externally** (heuristic only)
- Lowers dead-code deletion confidence for such symbols
- Outputs clear `cautionLevel`, `reasons`, `recommendedVerification`
- Integrates into existing tools (dead_code, reachability, review_plan, insights)

**Explicitly NOT:**
- Proof of external usage
- External registry query
- Safe-to-delete guarantee
- Automated deletion

## 3. New MCP Tool: `codelattice_external_api_surface`

### Input
```json
{
  "root": string (required),
  "language": "rust|cangjie|arkts|typescript|c|cpp|python|auto",
  "compact": bool (default true),
  "limit": int (default 50, max 200),
  "includeDocs": bool (default true),
  "includeTests": bool (default false),
  "includeHeaders": bool (default true),
  "includePackageMetadata": bool (default true)
}
```

### Output
```json
{
  "language": "typescript",
  "root": "...",
  "summary": {
    "externalSurfaceSymbolCount": 12,
    "externalSurfaceFileCount": 4,
    "packageExportCount": 3,
    "headerApiCount": 0,
    "documentedApiCount": 5,
    "highCautionCount": 6
  },
  "externalSurfaceSymbols": [...],
  "externalSurfaceFiles": [...],
  "generatedFrom": {
    "graphBased": true,
    "compilerVerified": false,
    "externalUsageVerified": false,
    "heuristic": true
  }
}
```

### MCP tool count: 31 → 32

## 4. Scoring Strategy

**Positive signals (add to score 0.0–1.0):**
| Signal | Delta |
|--------|-------|
| exported/public visibility | +0.30 |
| package/root entry file (lib.rs, index.ts, __init__.py) | +0.25 |
| re-exported from index/init/lib | +0.20 |
| documented in README/docs | +0.15 |
| header under include/ | +0.25 |
| package metadata references (package.json main/types/bin) | +0.25 |
| entry point candidate | +0.10 |

**Negative signals:**
| Signal | Delta |
|--------|-------|
| name starts _ or private/internal | -0.25 |
| test/generated/vendor path | exclude (unless includeTests) |

**cautionLevel thresholds:**
- score >= 0.75 → `high`
- score >= 0.45 → `medium`
- else → `low`

**Output filter:** only include score >= 0.35 (unless limit is high)

## 5. Language-Specific Heuristics

### Rust
- `pub` visibility → +0.30
- lib.rs items → +0.25
- `pub use` re-export → +0.20
- Caution: `rust-public-api-may-have-external-crate-consumers`

### TypeScript/ArkTS
- `export` keyword → +0.30
- index.ts / package entry → +0.25
- package.json exports/main/types/bin → +0.25
- TSX exported component → +0.20
- Caution: `typescript-package-export-may-have-downstream-consumers`

### Python
- `__init__.py` re-export → +0.25
- top-level non-underscore name → +0.20
- Caution: `python-package-api-may-have-external-importers`

### C
- declaration in .h under include/ → +0.30
- non-static function in header → +0.25
- Caution: `c-header-api-may-have-external-callers`

### C++
- declaration in .hpp under include/ → +0.30
- exported namespace/class in header → +0.25
- Caution: `cpp-header-api-may-have-external-callers`

### Cangjie
- public package symbol → +0.30
- package root export → +0.25
- Caution: `cangjie-public-api-may-have-external-consumers`

### ArkTS
- @Entry component → +0.30
- exported page/component → +0.25
- Caution: `arkts-component-entry-may-be-used-by-framework`

## 6. Integration Points

### dead_code_candidates
- Query external surface for each candidate
- If high caution: lower score by 0.20, cap confidence at "low"
- Add `external-consumer-usage-not-verified` caution
- Add `externalConsumerCautionCount` to summary

### reachability_map
- Add `externalConsumerCautions` to unreachable candidates
- Public API unreachable: don't mark as high-confidence dead

### review_plan
- release_check/before_edit: if changed symbol has external surface, add checklist item
- onboarding: optional public API surface summary

### project_insights
- Add external API signal to summary/riskMap
- reviewFirst: prioritize public API hotspots

## 7. Write Set / Forbidden Set

**Write (modify):**
- `crates/cli/src/mcp_server.rs` — new tool + integration
- `crates/cli/tests/mcp_server.rs` — new tests
- `scripts/mcp-dogfood.sh` — check #32
- `scripts/codelattice-mcp.sh` — threshold >= 32
- `CHANGELOG.md`, `README.md`, `docs/architecture/mcp-v0-contract.md`
- `docs/plans/` — this doc

**Forbidden:**
- GitNexus-RC / GitNexus-RC-Tool / CodeLattice-Tool
- Real project source code
- External registries
- Automated code deletion

## 8. Stop-Line

Stop and report if:
- Baseline tests fail
- Compilation errors after assembly
- Any test regression in existing tests
- `cargo fmt --check` fails

## 9. Verification Plan

1. `cargo check` — 0 errors
2. `cargo test --test mcp_server --features tree-sitter-typescript` — all pass
3. `scripts/mcp-dogfood.sh` — 32/32
4. `scripts/codelattice-mcp.sh --self-test` — pass
5. `cargo fmt --check` + `git diff --check` — clean
6. Real corpus smoke (if cache exists)
7. `git push gitcode master` — hooks pass
