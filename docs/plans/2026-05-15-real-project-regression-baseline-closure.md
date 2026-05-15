# Real Project Regression Baseline Closure

Date: 2026-05-15

## Summary

Completed the second block of the Multi-Language Production Hardening Pack:
the GitCode real-project corpus now has a saved regression baseline and a loose
quality budget gate.

## Delivered

- `docs/real-project-corpus-baseline.json`: saved Redis / Catch2 / pip metrics
  and default budget thresholds.
- `scripts/real-project-corpus-smoke.py`:
  - `--compare-baseline`
  - `--accept-baseline`
  - `--strict-baseline`
  - `--baseline <path>`
  - `--markdown-out <path>`
- `scripts/real-project-corpus-smoke-test.py`: standard-library unit tests for
  baseline pass / warn / fail / accept behavior.
- `docs/real-project-corpus.md`: baseline workflow and budget rules.
- `docs/release/smoke-matrix.md`: local corpus command now points at
  `--compare-baseline`.

## Baseline Budget

| Metric group | Warn | Fail |
|--------------|------|------|
| node / edge / symbol / file counts | 10% drop | 20% drop |
| elapsed runtime | 50% slower | 150% slower |

Warnings do not fail by default because local runtime can vary. Release gates
can opt into `--strict-baseline`.

## Real Corpus Verification

Ran with cached GitCode checkouts in `/tmp/codelattice-real-corpus-smoke`:

| Target | Status | Nodes | Edges | Symbols | Files |
|--------|--------|------:|------:|--------:|------:|
| `redis-c` | PASS | 10,967 | 11,486 | 10,751 | 133 |
| `catch2-cpp` | PASS | 7,522 | 21,155 | 7,076 | 225 |
| `pip-python` | PASS | 34,626 | 63,471 | 33,993 | 632 |

Generated reports:

- `/tmp/codelattice-real-corpus-results.json`
- `/tmp/codelattice-real-corpus-results.md`

## Verification Commands

- `python3 scripts/real-project-corpus-smoke-test.py`
- `python3 -m py_compile scripts/real-project-corpus-smoke.py scripts/real-project-corpus-smoke-test.py`
- `python3 scripts/real-project-corpus-smoke.py --help`
- `python3 scripts/real-project-corpus-smoke.py --offline --compare-baseline --cache-dir /tmp/codelattice-real-corpus-smoke --json-out /tmp/codelattice-real-corpus-results.json --markdown-out /tmp/codelattice-real-corpus-results.md`
- `python3 scripts/real-project-corpus-smoke.py --offline --accept-baseline --baseline /tmp/codelattice-real-corpus-accepted-baseline.json --cache-dir /tmp/codelattice-real-corpus-smoke --skip-insights`

## Boundaries

- Did not vendor target repositories.
- Did not modify target project sources.
- Did not run target build, package-manager, or test commands.
- Did not touch GitNexus-RC / GitNexus-RC-Tool / AI client configs.
- Did not change language adapter behavior.
