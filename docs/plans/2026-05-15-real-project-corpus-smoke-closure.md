# Real Project Corpus Smoke Closure

Date: 2026-05-15

## Summary

Added the first real-project corpus smoke layer for CodeLattice. The corpus is
GitCode-backed, cloned on demand, and intentionally kept out of the repository.

## Delivered

- `docs/real-project-corpus.json`: target registry with Tier 1 defaults and
  Tier 2 optional targets.
- `scripts/real-project-corpus-smoke.py`: standard-library runner for clone /
  cache / MCP overview / project insights / threshold checks.
- `docs/real-project-corpus.md`: usage guide and initial baseline metrics.
- Release smoke matrix updated with the new local-only corpus command.

## Baseline Results

Validated with `target/release/codelattice` at `597db2e`:

| Target | Language | Status | Nodes | Edges | Symbols | Files |
|--------|----------|--------|------:|------:|--------:|------:|
| `redis-c` | C | PASS | 10,967 | 11,486 | 10,751 | 133 |
| `catch2-cpp` | C++ | PASS | 7,522 | 21,155 | 7,076 | 225 |
| `pip-python` | Python | PASS | 34,626 | 63,471 | 33,993 | 632 |

## Verification

- `python3 -m py_compile scripts/real-project-corpus-smoke.py`
- `python3 scripts/real-project-corpus-smoke.py --list`
- `python3 scripts/real-project-corpus-smoke.py --dry-run --max-targets 2`
- `python3 scripts/real-project-corpus-smoke.py --target redis-c --cache-dir /tmp/codelattice-real-corpus-smoke`
- `python3 scripts/real-project-corpus-smoke.py --target catch2-cpp --cache-dir /tmp/codelattice-real-corpus-smoke`
- `python3 scripts/real-project-corpus-smoke.py --target pip-python --cache-dir /tmp/codelattice-real-corpus-smoke`

## Notes

The first corpus intentionally uses low minimum thresholds. The value is in
detecting empty output, command failure, feature drift, and large-project
runtime regressions. Later hardening packs can add baseline comparison windows,
per-language quality rates, and stricter regression budgets.
