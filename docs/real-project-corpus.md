# Real Project Corpus Smoke

CodeLattice uses fixtures for deterministic unit-level coverage, but fixtures do
not expose the awkward parts of real repositories: vendored code, unusual
directory layouts, mixed languages, generated files, and large symbol graphs.
The real project corpus smoke adds a second layer of evidence without vendoring
third-party projects into this repo.

## Command

```bash
python3 scripts/real-project-corpus-smoke.py --list
python3 scripts/real-project-corpus-smoke.py
```

By default the script clones enabled GitCode targets into:

```bash
${CODELATTICE_CORPUS_DIR:-${TMPDIR:-/tmp}/codelattice-real-project-corpus}
```

Override the cache directory when you want to reuse checkouts across sessions:

```bash
python3 scripts/real-project-corpus-smoke.py \
  --cache-dir "$HOME/Desktop/CodeLattice-Smoke-Targets"
```

The script is read-only with respect to target projects. It runs CodeLattice MCP
tools only; it never runs `make`, `npm`, `pip`, `pytest`, project scripts, or AI
client configuration writes.

## Default Targets

| ID | Language | GitCode URL | Purpose |
|----|----------|-------------|---------|
| `redis-c` | C | `https://gitcode.com/gh_mirrors/re/redis.git` | Mature C server codebase; scans `src/` to avoid vendored C++ deps. |
| `catch2-cpp` | C++ | `https://gitcode.com/gh_mirrors/ca/catch2.git` | Header-heavy C++ project with namespaces/classes/tests/templates. |
| `pip-python` | Python | `https://gitcode.com/gh_mirrors/pi/pip.git` | Large Python packaging codebase with packages/imports/tests. |

Optional Tier 2 targets cover TypeScript, ArkTS, Cangjie, and Rust. They are in
`docs/real-project-corpus.json` but are not enabled by default because they are
larger or more situational.

## Useful Runs

Run only one language:

```bash
python3 scripts/real-project-corpus-smoke.py --language python
```

Run a single target:

```bash
python3 scripts/real-project-corpus-smoke.py --target redis-c
```

Run all configured targets, including optional Tier 2:

```bash
python3 scripts/real-project-corpus-smoke.py --all
```

Use cached checkouts only:

```bash
python3 scripts/real-project-corpus-smoke.py --offline
```

Write machine-readable results:

```bash
python3 scripts/real-project-corpus-smoke.py \
  --json-out /tmp/codelattice-real-corpus-results.json
```

Compare against the saved regression baseline:

```bash
python3 scripts/real-project-corpus-smoke.py \
  --compare-baseline \
  --json-out /tmp/codelattice-real-corpus-results.json \
  --markdown-out /tmp/codelattice-real-corpus-results.md
```

Refresh the baseline after an intentional analyzer change:

```bash
python3 scripts/real-project-corpus-smoke.py --accept-baseline
```

## Initial Baseline

Saved in `docs/real-project-corpus-baseline.json` and refreshed on 2026-05-16
for the `0.14.0-beta.1` hardening pack with `target/release/codelattice`:

| Target | Status | Nodes | Edges | Symbols | Files | Quality summary |
|--------|--------|------:|------:|--------:|------:|-----------------|
| `redis-c` | PASS | 10,967 | 11,478 | 10,751 | 133 | dangling=0, unknownEdge=95.5% |
| `catch2-cpp` | PASS | 7,522 | 19,856 | 7,076 | 225 | dangling=0, unknownEdge=51.0% |
| `pip-python` | PASS | 34,626 | 61,989 | 33,993 | 632 | dangling=0, lowCall=35.3%, unknownEdge=47.8% |

These are smoke baselines, not precision guarantees. A future run should be
investigated if counts drop sharply, explode unexpectedly, or the command
starts failing.

The baseline budget is intentionally loose:

| Metric group | Warn | Fail |
|--------------|------|------|
| Count metrics (`nodeCount`, `edgeCount`, `symbolCount`, `sourceFileCount`) | 10% drop | 20% drop |
| Runtime (`elapsedSeconds`) | 50% slower | 150% slower |
| Quality rates in legacy baselines without stored qualityMetrics | >= 30% | >= 50% |
| Quality rate increase from stored baseline | +5 percentage points | +10 percentage points |
| Dangling edges (`danglingEdgeCount`) | — | > 0 |

Warnings keep the command successful by default so local hardware variance does
not block development. Use `--strict-baseline` when a release gate should treat
warnings as failures.

The quality rate comparison is baseline-relative once a target has stored
`qualityMetrics`. This matters for Phase A languages where high
unknown-confidence structural edges are an explicit limitation rather than a
new regression. Dangling edges remain a hard failure.

## Why This Exists

This corpus is the first piece of the Multi-Language Production Hardening Pack.
It answers a pragmatic question: after adding language adapters, can CodeLattice
survive real repositories and produce non-empty, stable graph metrics?

It does not replace compiler/IDE validation. It is a production-oriented smoke
gate for graph availability, output shape, and MCP stability.
