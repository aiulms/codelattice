# Dead Code Candidate Deep Research Pack — Preflight

**Date:** 2026-05-16
**Status:** Planning
**HEAD:** 12d6373

## What is a dead code candidate?

A symbol (function, method, class, struct, etc.) or file that appears unused based on static graph analysis — no incoming edges, not reachable from detected entry points. This is a **heuristic candidate**, not compiler-verified proof of death.

## Why NOT proof

- CodeLattice does not do control flow analysis, type inference, or macro expansion
- Dynamic dispatch, reflection, plugin systems, function pointers can hide callers
- Public/exported APIs may be called by external consumers not in the graph
- Build configs (cfg, features) may conditionally include code
- Test-only code may be critical for CI

## Supported scope

- Static graph-based reachability from detected entry points
- Per-symbol and per-file candidate scoring
- Confidence tiers (high/medium/low)
- Public API cautions
- Dynamic feature cautions
- Recommended verification steps

## Not supported

- Compiler-verified dead code elimination
- Control flow / reachability within functions
- Type-informed dispatch resolution
- Macro-expanded code paths
- Plugin / reflection / dynamic import resolution
- Automatic code deletion or patch generation

## Scoring strategy

### Symbol candidates (base score 0.0, add/subtract)

**Positive signals (likely dead):**
- No incoming CALLS/REFERENCES/IMPORTS/INCLUDES edges: +0.35
- Not reachable from any detected entry point: +0.25
- Private/internal visibility: +0.15
- Not mentioned in docs: +0.10
- File is orphan-like (no incoming file-level edges): +0.10
- Low fan-out / isolated: +0.05

**Negative signals (likely alive):**
- Public/exported symbol: -0.35
- Name looks entry-like (main, run, start, init, etc.): -0.40
- Test symbol when includeTests=false: exclude entirely
- Generated/vendor file: exclude or strong downweight
- Dynamic language caution pattern: -0.15
- Symbol kind is module/package/repository/file: exclude entirely

### File candidates

**Positive signals:**
- No incoming file-level import/include/reference edges: +0.35
- No entry-like symbols inside: +0.20
- Only candidate symbols inside: +0.20
- Not referenced by docs: +0.10
- Low outgoing edges: +0.05

**Negative signals:**
- Contains public API exports: -0.30
- Filename is entry-like (main.rs, index.ts, etc.): -0.40
- Tests/examples/fixtures when includeTests=false: exclude
- Generated/vendor/dist/build path: exclude

### Confidence mapping

- score >= 0.80 => "high"
- score >= 0.55 => "medium"
- else => "low"

Candidates below 0.45 are excluded from output by default.

### Entry point detection

Entry-like file names:
- main.rs, lib.rs, main.c, main.cpp, index.ts, index.tsx, app.ts, main.ts, main.py, app.py, api.py

Entry-like symbol names:
- main, run, start, init, create_app, handler, build, WinMain, DllMain

Language-specific:
- ArkTS: @Entry component, build method
- Cangjie: main, package root
- User-provided entryHints: exact symbol match or file path contains

Entry points themselves are NEVER dead code candidates.

Reachability traversal: from entry points, follow outgoing CALLS/REFERENCES/IMPORTS/INCLUDES/DEFINES edges. Max depth 8.

If no entry points detected: still produce no-incoming candidates, but emit warning `entry-point-detection-low-confidence`.

### Public API cautions

Signals:
- Rust `pub` visibility
- TypeScript `export` keyword
- Python `__init__.py` re-export
- C/C++ header-declared symbols
- File path under `include/`
- Symbol visibility property contains "public"

Effect: public API candidates get confidence capped at "medium", and cautions include `public-api-may-have-external-callers`.

### Dynamic feature cautions

Patterns (filename, symbol name, edge properties):
- Python: importlib, getattr, eval
- TypeScript: dynamic import, route/config-like files
- C/C++: function pointer-like low-confidence calls, macro-heavy files
- Rust: macro invocation, proc macro hints
- Generic: plugin, registry, route, command, handler, config, reflection

Effect: `-0.15` score penalty + caution `dynamic-dispatch-may-hide-callers`.

## Write set

- `crates/cli/src/mcp_server.rs` — add `compute_dead_code_candidates`, `handle_dead_code_candidates`, tool schema + dispatch
- `crates/cli/tests/mcp_server.rs` — add 9 new test functions
- `fixtures/typescript/dead-code-candidates/` — new fixture (7 files)
- `scripts/codelattice-mcp.sh` — update tool count threshold to >= 25
- `scripts/mcp-dogfood.sh` — add dead_code_candidates check, update count to 25
- `CHANGELOG.md` — Unreleased entry
- `README.md` — new section
- `docs/architecture/mcp-v0-contract.md` — new tool section
- `docs/plans/2026-05-16-dead-code-candidates-preflight.md` — this file

## Forbidden set

- No changes to GitNexus-RC, GitNexus-RC-Tool, CodeLattice-Tool
- No changes to language adapter crates (c, cpp, typescript, python, etc.)
- No new Cargo dependencies
- No code deletion / patch generation
- No deletionSafe=true in output
- No "proof" language in output — always "candidate"
- No WebUI
- No real project source modifications

## Stop-line

- If `cargo test --all-features` fails: stop, report, do not continue
- If `cargo fmt --check` fails: stop, fix, re-verify
- If any existing test breaks: stop, fix before continuing

## Verification plan

1. `cargo fmt --check`
2. `git diff --check`
3. `cargo test --test mcp_server` (default features)
4. `cargo test --test mcp_server --features tree-sitter-typescript`
5. `cargo test --all-features`
6. `scripts/codelattice-mcp.sh --self-test`
7. `scripts/mcp-dogfood.sh`
8. Real project corpus smoke (if cache available)
9. Tool detect-changes
10. Tool index refresh
