# Real Project Corpus Smoke Preflight

Date: 2026-05-15

## Goal

Add a maintainable real-project smoke layer for the Multi-Language Production
Hardening Pack. The first target is not deeper language semantics; it is a
repeatable way to check that CodeLattice can scan real GitCode projects without
panic, empty output, or language feature drift.

## Scope

Write set:

- `docs/real-project-corpus.json`
- `docs/real-project-corpus.md`
- `scripts/real-project-corpus-smoke.py`
- `docs/release/smoke-matrix.md`
- `docs/plans/README.md`
- `CHANGELOG.md`
- closure doc for this pack

Forbidden set:

- No vendored third-party repositories in CodeLattice.
- No target project source modification.
- No build/test/package manager commands inside target projects.
- No GitNexus-RC runtime/schema/WebUI changes.
- No AI client config changes.

## Design

The corpus is configured as JSON and consumed by a Python smoke runner using
only the standard library. The runner:

1. selects targets by default / target id / language / tier;
2. clones missing GitCode repositories into a cache directory;
3. optionally updates cached checkouts;
4. calls CodeLattice MCP tools directly through the selected `codelattice`
   binary;
5. runs `codelattice_project_overview` and `codelattice_project_insights`;
6. checks minimum non-empty graph thresholds;
7. prints human-readable output and optional JSON results.

Default Tier 1 focuses on the three newest language adapters:

- C: Redis (`redis-c`, project path `src/`)
- C++: Catch2 (`catch2-cpp`)
- Python: pip (`pip-python`)

Tier 2 keeps optional GitCode targets for TypeScript, ArkTS, Cangjie, and Rust.

## Risk

| Risk | Mitigation |
|------|------------|
| GitCode network or repo availability changes | `--offline`, `--cache-dir`, and target-level filtering. |
| Large repos make default smoke too slow | Default only runs three Tier 1 targets; optional targets require explicit filters or `--all`. |
| Real project counts are noisy | Thresholds are low; exact counts are documented as baseline evidence, not hard contracts. |
| Mixed C/C++ repos break pure C detection | Redis target uses `projectPath=src` to avoid vendored C++ deps. |

## Verification Plan

- `python3 -m py_compile scripts/real-project-corpus-smoke.py`
- `python3 scripts/real-project-corpus-smoke.py --list`
- `python3 scripts/real-project-corpus-smoke.py --dry-run --max-targets 2`
- real GitCode target runs for `redis-c`, `catch2-cpp`, and `pip-python`
- `cargo fmt --check`
- `git diff --check`
- `cargo test`
- GitNexus `detect-changes`
