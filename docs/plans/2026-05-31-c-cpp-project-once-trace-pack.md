# C/C++ Project-Once Trace Pack

Date: 2026-05-31

## Goal

Make C and C++ behave like the other project-level analyzers in MCP job mode:

- one project-level analysis task per project job,
- no per-file task fan-out for normal facade project jobs,
- stage-level trace visible to AI agents,
- bounded source reads and parse passes.

## Current Gap

The C/C++ CLI path now parses each file once, but the MCP job bridge still does not advertise C/C++ as project-level adapters. That means C/C++ jobs can fall back to less direct engine behavior and do not expose the same `analysisTrace` contract used by TypeScript, JavaScript, and Python.

## Planned Change

1. Add `run_c_analysis_with_trace` and `run_cpp_analysis_with_trace`.
2. Make both traces use `codelattice.languageAnalysisTrace.v1` with:
   - `parsePassesPerFile = 1`
   - `sourceReadPasses = 1`
   - extraction / call extraction / graph / serialization timing
3. Register C/C++ in `engine_bridge::run_project_analysis_once`.
4. Add lightweight C/C++ engine adapters for file discovery and capability metadata.
5. Add MCP job tests proving `executor_mode=project-once` and trace availability.

## Boundaries

- No graph schema changes.
- No MCP tool count changes.
- No target code execution, build execution, package manager execution, or `compile_commands` command execution.
- No live repo modifications.
- No installed `CodeLattice-Tool` sync.

