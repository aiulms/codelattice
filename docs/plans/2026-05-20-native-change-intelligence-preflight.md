# Preflight: CodeLattice Native Change Intelligence Pack

**Date**: 2026-05-20
**Base**: HEAD=`6a07a72` (facade consolidation)
**Target commit**: `feat(governance): add native change intelligence review`

## 1. Current State Analysis

### detect-changes CLI (`lib.rs` L2904-3016)
- CLI subcommand: `Commands::DetectChanges { root, language, scope, diff_mode, base_ref, format, limit, compact, include_snippet, snippet_context }`
- Flow: `check_root → resolve_language → scope_to_diff_mode → call codelattice_changed_symbols → call codelattice_production_assist → collect_untracked_files → build_detect_changes_report → println JSON`
- Uses `call_mcp_tool_via_current_binary()` to invoke MCP tools via subprocess
- **No workspace integration exists** — pure single-project analysis

### Current output schema (`codelattice.detectChanges.v1`)
```json
{
  "schemaVersion": "codelattice.detectChanges.v1",
  "root": "...",
  "language": "...",
  "scope": "...",
  "diffMode": "...",
  "summary": {
    "changedFileCount": 0,
    "changedSymbolCount": 0,
    "unknownHunkCount": 0,
    "deletedFileCount": 0,
    "renamedFileCount": 0,
    "untrackedFileCount": 0,
    "totalFileChangeCount": 0,
    "riskLevel": "low|medium|high|critical",
    "affectedProcessCount": null,
    "affectedProcessModel": "notAvailable"
  },
  "changedFiles": [],
  "changedSymbols": [],
  "unknownHunks": [],
  "deletedFiles": [],
  "renamedFiles": [],
  "untrackedFiles": [],
  "risk": { "overallRisk": "...", "overallRiskReasons": [], "highestRiskSymbols": [] },
  "reviewChecklist": [],
  "quality": { ... },
  "docs": { ... },
  "generatedFrom": { staticAnalysis: true, runtimeVerified: false, ... nativeCodeLattice: true },
  "cautions": [],
  "underlyingTools": ["codelattice_changed_symbols", "codelattice_production_assist"]
}
```
- Compact mode removes: quality, docs, deletedFiles, renamedFiles

### workspace-model crate (`crates/workspace-model/`)
- `pub fn build_workspace_graph(root, redact_root) -> Result<WorkspaceGraph, String>` (L378)
- `pub fn scan_workspace_inventory(root, redact_root) -> Result<Vec<ProjectInfo>, String>` (L152)
- `impact::cross_project_impact(graph, target, direction, max_depth) -> ImpactResult`
- Already used in MCP: `handle_workspace_graph` (L16223), `handle_cross_project_impact` (L16253)
- `cli/Cargo.toml` already depends on `gitnexus-workspace-model`

### Risk grading (`pick_detect_changes_risk`, L395)
- Takes `changed_symbols` risk + `production_assist` risk, picks max
- `severity_rank`: critical=4, high=3, medium=2, low=1, _=0

### Precommit script (`scripts/codelattice-precommit-check.sh`, 163 lines)
- Runs: cargo fmt, git diff --check, focused tests, smoke, detect-changes
- `--fail-on-high-risk` flag exists (exits 1 on high/critical)
- Writes report to `/tmp/codelattice-precommit-detect-changes.json`
- Prints summary via Python

### Existing workspace fixture (`fixtures/workspace/`)
- `rust-core/Cargo.toml` — Rust project
- `ts-ui/package.json` — TypeScript project
- `unsupported-csharp/demo.csproj` — unsupported language
- `scripts/{build-core.sh,deploy.sh}` — shell scripts
- `Dockerfile`, `Makefile` — config nodes
- `.github/workflows/` — CI configs
- `multi-project/` — another workspace with similar structure (but empty dirs!)

## 2. Design Decisions

### D1: Schema version bump
- Bump from `codelattice.detectChanges.v1` → `codelattice.detectChanges.v2`
- All v1 fields preserved; new fields are additive
- Compact mode unchanged for existing fields

### D2: File ownership mapping
- New function `map_files_to_projects(changed_files, workspace_graph) -> Vec<FileOwner>`
- Logic: for each changed file path, find the WorkspaceNode with the longest matching `relative_path` prefix
- Node kinds: project → ownerKind=project; config → ownerKind=config; script → ownerKind=script
- Files under unsupported project → ownerKind=unsupported
- Files not matching any node → ownerKind=unknown

### D3: Workspace integration in detect-changes
- After collecting changed + assist data, try `build_workspace_graph(root, true)`
- If workspace graph succeeds: compute file owners + cross-project impact
- If workspace graph fails (single project, no workspace): graceful degrade with empty arrays + reason
- New fields added to report: `workspaceContext`, `affectedProjects`, `affectedWorkspaceEdges`, `unsupportedBoundaryHits`, `crossProjectRisk`, `riskReasons` (enhanced), `recommendedFollowups`, `fileOwners`

### D4: Cross-project impact via workspace graph
- For each owned project that has changed, run `cross_project_impact` with that project as target
- Merge results: deduplicated affectedProjects, affectedAssets, unsupported boundaries
- Compute crossProjectRisk from the merged impact

### D5: Risk reasons enhancement
- Existing risk reasons from `production_assist` preserved
- New reasons from workspace analysis:
  - `changed_public_api_surface` — if changed symbol is in external API surface
  - `changed_framework_entry` — if changed symbol is a framework entry hint
  - `changed_config_script_ci` — if changed file is config/script/CI
  - `unknown_file_owner` — if changed file has no project owner
  - `downstream_workspace_dependents` — if changed project has workspace-level dependents
  - `adjacent_to_unsupported_module` — if changed project is adjacent to unsupported language node
  - `many_unknown_hunks` — if unknownHunkCount > changedSymbolCount
  - `no_symbols_detected_but_files_changed` — if changedFileCount > 0 but changedSymbolCount == 0

### D6: Precommit script upgrade
- Add workspace-aware detect-changes fields to Python summary
- Print Chinese-friendly terminal summary
- Output JSON to `/tmp/codelattice-change-review.json`
- Keep `--fail-on-high-risk` behavior

### D7: MCP facade integration
- Update `codelattice_change_review` with new mode `native_review`
- `full_review` mode includes workspace fields when available
- No new standalone MCP tool needed

### D8: Fixture for testing
- Extend `fixtures/workspace/multi-project/` with actual files (currently empty dirs)
- Needs: Rust project, TS project, shell scripts, CI config, unsupported C# project, config referencing project
- Create a git repo fixture for detect-changes testing

## 3. Implementation Plan (Ordered)

### Phase 1: Core Rust Changes (lib.rs)
1. **Add workspace imports** — import `build_workspace_graph`, `cross_project_impact`, etc.
2. **New function: `map_files_to_projects`** — file path → project owner mapping
3. **New function: `compute_workspace_impact`** — orchestrates graph build + file owners + cross-project BFS
4. **Enhance `build_detect_changes_report`** — add new fields, keep all existing
5. **Update `Commands::DetectChanges` handler** — call workspace integration after existing analysis
6. **Bump schema version** to v2

### Phase 2: Risk Enhancement
7. **New function: `compute_enhanced_risk_reasons`** — workspace-aware risk reasons
8. **New function: `compute_recommended_followups`** — AI/human action suggestions
9. **Integrate into `build_detect_changes_report`**

### Phase 3: Precommit Script
10. **Update `scripts/codelattice-precommit-check.sh`** — workspace-aware summary, Chinese output

### Phase 4: MCP Facade
11. **Add `native_review` mode to `codelattice_change_review` facade**
12. **Update `full_review` mode to include workspace fields**

### Phase 5: Fixtures & Tests
13. **Populate `fixtures/workspace/multi-project/`** with actual files
14. **Update `scripts/codelattice-detect-changes-smoke.sh`** — test new fields
15. **Update `scripts/codelattice-mcp-facade-smoke.sh`** — test native_review mode
16. **Update Rust test: tool count still 50**

### Phase 6: Documentation
17. **Update README.md, CHANGELOG.md, contract docs**
18. **Write preflight + closure docs**

### Phase 7: Verification & Commit
19. **Run full verification suite per spec**
20. **Commit and push**

## 4. Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| `build_workspace_graph` fails on single-project repos | Medium | Graceful degrade: empty arrays + reason string |
| File path matching ambiguity (nested projects) | Low | Use longest-prefix match with confidence scoring |
| Cross-project BFS too expensive for large workspaces | Low | `MAX_WALK_DEPTH=5` already limits traversal |
| Breaking v1 schema consumers | High | All new fields are additive; v1 fields unchanged |
| `lib.rs` already 3026 lines, workspace code adds ~200+ | Medium | Keep workspace logic in separate functions; no single function >50 lines |

## 5. File Change Budget

| File | Change Type | Estimated Lines |
|------|-------------|----------------|
| `crates/cli/src/lib.rs` | Modify | +250 (workspace integration functions + enhanced report) |
| `crates/cli/src/mcp_server.rs` | Modify | +60 (native_review mode) |
| `scripts/codelattice-precommit-check.sh` | Modify | +40 (enhanced summary) |
| `scripts/codelattice-detect-changes-smoke.sh` | Modify | +30 (new field checks) |
| `scripts/codelattice-mcp-facade-smoke.sh` | Modify | +20 (native_review test) |
| `fixtures/workspace/multi-project/` | Create | ~80 (fixture files) |
| `CHANGELOG.md` | Modify | +5 |
| `README.md` | Modify | +5 |
| `docs/plans/2026-05-20-native-change-intelligence-preflight.md` | Create | this file |
| `docs/plans/2026-05-20-native-change-intelligence-closure.md` | Create | post-implementation |

**Total estimated**: ~500 new/modified lines across ~10 files

## 6. Stop-lines

- No LLM / embedding / runtime execution
- No modification to GitNexus-RC / Tool / CodeLattice-Tool
- No deletion of existing tools or fields
- No breaking schema changes
- Graceful degradation on workspace errors, never hard fail
