# CodeLattice Real Corpus Baseline Report

Date: 2026-05-16
Version: `0.14.0-beta.1`
Source: `docs/real-project-corpus-baseline.json`
Live compare: `/tmp/codelattice-beta-real-corpus.json`, `/tmp/codelattice-beta-real-corpus.md`

## Summary

This report summarizes the real-project corpus baseline used as beta evidence for `0.14.0-beta.1`.

The baseline is a smoke/regression signal, not a precision proof. It checks that graph size, runtime, quality budgets, and dangling-edge behavior remain within expected bounds on real repositories. Target repositories are not vendored into CodeLattice, and CodeLattice does not run target project build/test/package scripts.

The baseline was refreshed during this release pack after the C/C++/Python/TypeScript hardening work landed. Quality rates are now stored per target, so future strict compares detect quality regression relative to each language's current beta state instead of treating all high unknown-confidence rates as absolute failures.

## Default Targets

| Target | Project | Language | nodeCount | edgeCount | symbolCount | sourceFileCount | qualityMetrics summary | elapsedSeconds |
|--------|---------|----------|----------:|----------:|------------:|----------------:|------------------------|---------------:|
| redis-c | Redis | C | 10967 | 11478 | 10751 | 133 | dangling=0, lowCall=0.0%, lowEdge=0.0%, unknownEdge=95.5%, callEdges=0 | 3.17 |
| catch2-cpp | Catch2 | C++ | 7522 | 19856 | 7076 | 225 | dangling=0, lowCall=0.0%, lowEdge=0.0%, unknownEdge=51.0%, callEdges=9726 | 13.71 |
| pip-python | pip | Python | 34626 | 61989 | 33993 | 632 | dangling=0, lowCall=35.3%, lowEdge=35.2%, unknownEdge=47.8%, callEdges=32310 | 30.72 |

## Live Strict Compare

The cached offline strict compare passed on 2026-05-16:

| Target | Language | Status | Baseline | Nodes | Edges | Symbols | Files | Quality | Elapsed |
|--------|----------|--------|----------|------:|------:|--------:|------:|---------|--------:|
| redis-c | C | pass | pass | 10967 | 11478 | 10751 | 133 | unknownEdge=95.5% | 3.55s |
| catch2-cpp | C++ | pass | pass | 7522 | 19856 | 7076 | 225 | unknownEdge=51.0% | 13.63s |
| pip-python | Python | pass | pass | 34626 | 61989 | 33993 | 632 | lowCall=35.3%, unknownEdge=47.8% | 30.97s |

## Optional Targets

If local cache exists, the corpus smoke can also verify:

| Target | Language | Status |
|--------|----------|--------|
| vite-typescript | TypeScript | Optional cached compare target |
| codelattice-rust | Rust | Optional cached compare target |
| cangjie-magic | Cangjie | Optional cached compare target |
| openharmony-app-samples-arkts | ArkTS | Optional cached compare target |

## Budgets

| Budget | Value |
|--------|------:|
| countDropWarnPercent | 10.0 |
| countDropFailPercent | 20.0 |
| elapsedIncreaseWarnPercent | 50.0 |
| elapsedIncreaseFailPercent | 150.0 |

The smoke harness also has default quality budgets for live compare:

| Quality Budget | Default |
|----------------|--------:|
| qualityRateWarnThreshold | 0.30 |
| qualityRateFailThreshold | 0.50 |
| qualityRateIncreaseWarnPoints | 0.05 |
| qualityRateIncreaseFailPoints | 0.10 |
| danglingEdgeFailThreshold | 0 |

`qualityRateWarnThreshold` and `qualityRateFailThreshold` apply to legacy baseline entries that do not store `qualityMetrics`. Once a target stores quality rates, strict compare uses the baseline-relative increase budgets. Dangling edges are always absolute.

## Live Compare Command

When cache is present:

```bash
python3 scripts/real-project-corpus-smoke.py \
  --offline \
  --compare-baseline \
  --strict-baseline \
  --cache-dir /tmp/codelattice-real-corpus-smoke \
  --json-out /tmp/codelattice-beta-real-corpus.json \
  --markdown-out /tmp/codelattice-beta-real-corpus.md
```

If the cache is absent, use:

```bash
python3 scripts/real-project-corpus-smoke.py --list
python3 scripts/real-project-corpus-smoke.py --dry-run --max-targets 3
```

## Notes

- Real corpus projects are external verification inputs, not release contents.
- The baseline should be updated only after intentional analyzer changes using `--accept-baseline`.
- Count increases are usually informational; count drops and dangling edges are higher-risk regression signals.
- High unknown-confidence rates in C/C++/Python are beta limitations, not precision claims. They remain visible in the report and are guarded as regression budgets.
- The final live compare result for this release pack is recorded in `docs/plans/2026-05-16-release-beta-hardening-closure.md`.
