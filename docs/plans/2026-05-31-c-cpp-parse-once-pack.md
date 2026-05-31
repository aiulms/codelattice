# C/C++ Parse-Once Pack

Date: 2026-05-31

## Goal

Reduce avoidable per-file parser work in the C and C++ adapters. This continues the same analysis-runtime direction already applied to TypeScript, Python, and ArkTS: parse a source file once, then reuse the tree for symbols, includes, and calls where possible.

## Current Friction

- C analysis parses each file once for symbols and again for includes.
- C++ analysis parses each file for symbols, again for includes, then rereads and reparses each file for call extraction after building the project function index.
- This is pure adapter overhead. It does not improve graph quality and makes cold analysis less friendly for AI tool use.

## Planned Change

1. Add root-based extractor APIs:
   - `extract_c_symbols_from_root`
   - `extract_c_includes_from_root`
   - `extract_cpp_symbols_from_root`
   - `extract_cpp_includes_from_root`
   - `extract_cpp_calls_from_root`
2. Add combined per-file APIs:
   - `extract_c_file`
   - `extract_cpp_file_base`
3. Update CLI C analysis to call the combined C extractor once per file.
4. Update CLI C++ analysis to read and parse each file once, extract symbols/includes immediately, keep the parsed tree in memory, then extract calls from the stored tree after the project function index is available.

## Boundaries

- Do not change graph schema.
- Do not change MCP tool count.
- Do not execute target project code, package managers, build scripts, or `compile_commands` commands.
- Do not modify live repos such as open-nwe or cangjie.
- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.

## Verification

- Add extractor equivalence tests proving root-based extraction matches existing source-based extraction.
- Run C/C++ focused tests with parser features.
- Run `cargo fmt --check`, `git diff --check`, relevant MCP tests, full `cargo test`, and native precommit.

