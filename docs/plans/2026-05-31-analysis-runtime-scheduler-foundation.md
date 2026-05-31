# Analysis Runtime Scheduler Foundation

Date: 2026-05-31

## Goal

Make CodeLattice feel reliable and fast for AI agents by fixing the runtime
foundation rather than only patching one slow language.

The current blind spot is architectural: MCP jobs are presented as queued, but
queued jobs are not actually scheduled after active jobs finish. Separately,
the Analysis Engine bridge exposes file-level tasks while several adapters call
project-level analyzers from each file task. That can multiply a single project
analysis by the number of files.

## Write Set

- `crates/cli/src/mcp_job.rs`
- `crates/cli/src/mcp_server.rs`
- `crates/cli/src/engine_bridge.rs`
- `crates/cli/tests/mcp_server.rs`
- this plan

## Forbidden Set

- Do not modify live repositories such as `open-nwe`.
- Do not modify GitNexus-RC runtime/schema.
- Do not sync `CodeLattice-Tool` until tests pass and the user asks.

## Stop Lines

- Stop if default AI toolset changes from 6 or full toolset changes from 49.
- Stop if queued jobs cannot be proven to transition to running/succeeded.
- Stop if TypeScript project jobs still execute per-file project analysis.
- Stop if root guard breaks `codelattice_project` workspace auto-entry.

## Design

### 1. True Job Queue

Move concurrency control into `mcp_job` job submission rather than the MCP
request loop. A job submission either starts immediately or is placed in a FIFO
queue. When an active analysis worker exits, the guard starts the next queued
job if capacity is available.

Control-plane calls (`job_status`, `job_detail`, `job_cancel`, cache status)
remain immediate.

### 2. Project-Level Analyzer Once

For adapters whose capability says `file_granularity=false`, the project job
must call the project analyzer once and store a project graph artifact. It must
not create `N files × stages` tasks that call `run_*_analysis(file.parent())`.

This applies to Rust, TypeScript, JavaScript, Python, and other project-level
adapters. Rust already has a facade warm fast path; this makes the job runtime
itself consistent.

### 3. Workspace Root Guard

`codelattice_symbol` and `codelattice_change_review` must reject workspace roots
instead of silently resolving `language=auto` to an incidental language such as
Python. The error should include primary project root recommendations.

## Verification

- Red/green tests for queued job execution with max analysis jobs set to 1.
- Test that TypeScript job summary reports one project-level task, not per-file
  tasks.
- Test that symbol on a workspace root returns a root-selection error.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`,
  and `scripts/codelattice-installed-acceptance.sh --dev-only`.
