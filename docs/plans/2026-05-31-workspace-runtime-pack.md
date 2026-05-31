# Workspace Runtime Pack

Date: 2026-05-31

## Goal

Make the workspace root path the AI-friendly default entry: when an agent passes
a monorepo root, CodeLattice should identify real manifest-backed projects,
analyze those projects with the same project-once runtime used by project jobs,
and return compact project digests instead of noisy file-task details.

## Write Set

- `crates/cli/src/mcp_job.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- this plan

## Forbidden Set

- Do not edit live repositories such as `open-nwe`.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` until the dev
  version passes and the user explicitly asks.
- Do not change MCP tool counts.

## Design

### 1. Manifest-backed project selection

Workspace jobs should prefer supported projects with manifests
(`Cargo.toml`, `package.json`, `pyproject.toml`, etc.). Source-only areas remain
reported as context, but they are not treated as independent analysis projects
when manifest-backed projects exist.

If a workspace has no manifest-backed supported project, keep the existing
source-only fallback so small script/source-only directories still get a useful
answer.

### 2. Project-once execution

Each selected project must call `run_project_analysis_once(project_root,
language)` exactly once. The workspace worker must not build per-file
`Parse/Symbol` task plans for project-level adapters.

### 3. AI digest detail

`job_detail` items should be project-level cards:

- project/name/path/language
- manifestFile / manifestBacked
- sourceFileCount
- symbolCount
- callEdgeCount
- nodeCount / edgeCount
- executorMode = `project-once`
- status / durationMs / facadeCacheReady

### 4. Compact summary

Workspace job summary should include counts for analyzed manifest projects,
skipped source-only areas, unsupported entries, total symbols, total call edges,
and a small `sourceOnlySummary` without expanding every source-only path.

## Stop Lines

- Stop if default AI toolset changes from 6 or full toolset changes from 49.
- Stop if existing workspace job compact/detail tests fail.
- Stop if workspace job detail still exposes parse/symbol task cards.
- Stop if source-only entries are analyzed by default when manifest-backed
  projects exist.

## Verification

- Add a failing MCP test using a temporary workspace with two Rust
  manifest-backed projects and one source-only scripts directory.
- Verify `job_detail` has two project cards, both `executorMode=project-once`.
- Verify `summary.sourceOnlySkippedCount=1`.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`,
  `cargo test`, `scripts/codelattice-installed-acceptance.sh --dev-only`, and
  `scripts/codelattice-precommit-check.sh`.
