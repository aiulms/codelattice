# C/C++ Include + compile_commands Refinement — Preflight / Execution Card

**Created**: 2026-05-16
**Status**: In Progress
**Depends on**: `c4e6805 feat(typescript): resolve tsconfig paths and workspace imports`

## Scope

### Supported
- `compile_commands.json` parsing (array format, `command` string + `arguments` array)
- Extract `-I`, `-isystem`, `-iquote`, `-D`, `-include` from compile commands
- Per-file include path lookup (compile command → source file match)
- Include resolution for `#include "x.h"`: same-dir → -iquote → -I → project-root
- Include resolution for `#include <x.h>`: check -I for project headers only, else system
- Forced include edges (`-include` flag)
- Confidence tiers: same-dir 0.95, quote-dir 0.90, project-dir 0.85, angle-via-project 0.75, forced 0.70, filename-unique 0.60
- Diagnostics for unresolved, ambiguous includes
- No dangling edges (fix existing C++ `unresolved:` synthetic targets)
- Both C and C++ crates updated

### Not Supported
- No compiler execution
- No system header reading (`/usr/include`, SDK)
- No macro expansion / `#ifdef` evaluation
- No full preprocessing
- No type inference
- No `compile_flags.txt` (only compile_commands.json)
- No Makefile / CMake parsing for include dirs
- No `-imsvc`, `-idirafter`, `-iprefix` or other exotic flags
- No response file (`@file`) expansion

## compile_commands Parsing Strategy
1. Read `compile_commands.json` from project root
2. Parse JSON array of entries
3. Each entry: `directory`, `file`, `command` or `arguments`
4. Prefer `arguments` over `command` when both present
5. Light shell splitting for `command` strings (spaces, single/double quotes)
6. Extract flags: `-I<dir>`, `-I <dir>`, `-isystem<dir>`, `-isystem <dir>`, `-iquote<dir>`, `-iquote <dir>`, `-DNAME`, `-DNAME=VAL`, `-include<file>`, `-include <file>`
7. Resolve paths relative to entry's `directory` (fallback to project root)
8. Build per-file lookup: `BTreeMap<PathBuf, CompileCommandEntry>`

## Include Resolution Strategy
For `#include "path.h"` (Local):
1. Same directory as source file
2. `-iquote` directories from compile command
3. `-I` project directories from compile command
4. Project root
5. Filename-unique fallback (only if exactly 1 match in project headers)

For `#include <path.h>` (System):
1. Check `-I` directories for project header match (confidence 0.75)
2. Else mark as System/External, no edge

For forced includes (`-include` flag):
- Resolve against compile command directory / include dirs
- Create edge if project file exists (confidence 0.70)

Ambiguous resolution (multiple candidates):
- No edge, diagnostic `c-include-ambiguous` / `cpp-include-ambiguous`

## C / C++ Differences
- C crate: `CInclude`, `CIncludeKind`, `build_c_graph(project, symbols, includes)`
- C++ crate: `CppInclude`, `CppIncludeKind`, `build_cpp_graph(project, symbols, includes, calls)`
- C++ has `find_include_target()` helper — replace with resolver
- C has inline filename matching — replace with resolver
- C++ currently creates `unresolved:` synthetic targets — MUST FIX (violates no-dangling-edge)
- Both need `compile_commands.rs` + `include_resolution.rs`
- Data structures are nearly identical — implement in each crate separately (no workspace crate)

## Edge Confidence / Reason Strategy

| Resolution Kind | Confidence | C Reason | C++ Reason |
|---|---|---|---|
| Same directory | 0.95 | `c-local-include-same-directory` | `cpp-local-include-same-directory` |
| -iquote dir | 0.90 | `c-quote-include-dir` | `cpp-quote-include-dir` |
| -I project dir | 0.85 | `c-project-include-dir` | `cpp-project-include-dir` |
| Angle via project -I | 0.75 | `c-project-include-angle-resolved` | `cpp-project-include-angle-resolved` |
| Forced include | 0.70 | `c-forced-include` | `cpp-forced-include` |
| Filename unique | 0.60 | `c-filename-unique-fallback` | `cpp-filename-unique-fallback` |
| System/External | N/A | no edge, diagnostic | no edge, diagnostic |
| Unresolved | N/A | `c-include-unresolved` | `cpp-include-unresolved` |
| Ambiguous | N/A | `c-include-ambiguous` | `cpp-include-ambiguous` |

## Write Set

### New Files
- `crates/c/src/compile_commands.rs` — compile_commands.json parser
- `crates/c/src/include_resolution.rs` — C include resolver
- `crates/c/tests/include_compile_commands.rs` — C crate tests
- `crates/cpp/src/compile_commands.rs` — compile_commands.json parser (similar to C)
- `crates/cpp/src/include_resolution.rs` — C++ include resolver
- `crates/cpp/tests/include_compile_commands.rs` — C++ crate tests
- `fixtures/c/include-compile-commands/` — C fixture
- `fixtures/cpp/include-compile-commands/` — C++ fixture

### Modified Files
- `crates/c/src/lib.rs` — expose new modules
- `crates/c/src/graph.rs` — use resolver, add diagnostics, fix include edges
- `crates/cpp/src/lib.rs` — expose new modules
- `crates/cpp/src/graph.rs` — use resolver, add diagnostics, remove `unresolved:` targets
- `crates/cli/src/lib.rs` — build resolvers, pass to graph builders
- `crates/cli/tests/mcp_server.rs` — new C/C++ MCP tests
- `CHANGELOG.md` — Unreleased entry

### Forbidden Set
- No GitNexus-RC / GitNexus-RC-Tool modifications
- No CodeLattice-Tool modifications
- No new dependencies
- No compiler execution
- No system header reading
- No dangling edges
- No C++ `unresolved:` synthetic targets
- No TS/Python changes
- No WebUI

## Stop-Line
- Baseline failure → stop and report
- cargo fmt/check failure → fix before continuing
- If compile_commands.json doesn't exist → fallback to old behavior (backward compat)

## Verification Plan
1. `cargo fmt --check`
2. `git diff --check`
3. `cargo test -p gitnexus-c --features tree-sitter-c`
4. `cargo test -p gitnexus-cpp --features tree-sitter-cpp`
5. `cargo test --test mcp_server --features tree-sitter-c,tree-sitter-cpp`
6. `cargo test --all-features`
7. `python3 scripts/real-project-corpus-smoke-test.py`
8. Real corpus dry-run (redis-c, catch2-cpp)

## Expected Impact on Quality Metrics
- `includeEdgeCount` should increase (more includes resolved via -I paths)
- `danglingEdgeCount` should decrease (C++ `unresolved:` targets eliminated)
- `unresolvedImportOrIncludeCount` should become more accurate
- C crate currently has no confidence on include edges → will gain proper confidence
