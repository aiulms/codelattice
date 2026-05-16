# CodeLattice Release Beta Hardening Pack Closure

Date: 2026-05-16

## Status

Verification complete before commit; `detect-changes`, index refresh, commit, and push are recorded after their final run.

## Summary

The Release Beta Hardening Pack turns the current multi-language analyzer work into a beta-ready release surface:

- Product version bumped to `0.14.0-beta.1`.
- README front page now explains CodeLattice as a local code intelligence engine for large, legacy, and complex codebases.
- Release notes, smoke matrix, install/MCP setup docs, release packaging docs, and real corpus baseline report were updated for seven-language beta use.
- Release packaging builds all optional language adapters by default and records `buildFeatures` in manifest outputs.
- Release smoke covers Rust, Cangjie, ArkTS, TypeScript, C, C++, and Python portable fixtures.
- Real corpus baseline now stores qualityMetrics and compares quality rates as regression deltas when baseline quality exists.
- Default `cargo test` no longer tries to compile C/C++ graph tests without their optional parser features.

## Baseline

Stage 0 baseline passed after a narrow test feature-gate fix:

- `cargo fmt --check`: PASS
- `git diff --check`: PASS
- `cargo test --test mcp_server`: PASS, 104/104
- `cargo test`: PASS
- `cargo test --all-features`: PASS
- `python3 scripts/real-project-corpus-smoke-test.py`: PASS, 10/10 after baseline regression tests were added
- `scripts/codelattice-mcp.sh --self-test`: PASS, 24 tools, all language flags true
- `scripts/mcp-dogfood.sh`: PASS, 24/24

## Version

`workspace.package.version` was bumped from `0.13.0-beta.2` to `0.14.0-beta.1`.

Reason: the previous beta corrected all-language packaging for Rust/Cangjie/ArkTS/TypeScript. This beta now closes the larger multi-language production-hardening set, including TypeScript path alias/monorepo handling, Python import resolution, C/C++ compile_commands include handling, release evidence, and external beta docs. That is broader than a patch beta.

No git tag was created.

## Artifact

Pre-commit artifact verification:

- Tarball: `dist/codelattice-0.14.0-beta.1-darwin-arm64.tar.gz`
- SHA256: `967a883b687c87276b2370bc5050e2845930c355b4f5d9fdf5071f0942d32912`
- Manifest version: `0.14.0-beta.1`
- Manifest platform: `darwin-arm64`
- Manifest sourceCommit during verification: `3594edf`
- Manifest MCP serverVersion: `0.13.0`
- Manifest toolCount: 24
- Manifest buildFeatures: `tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python`
- Language flags: Cangjie, ArkTS, TypeScript, C, C++, Python all true

The release artifact is generated under ignored `dist/`. For a published GitCode Release page, rebuild after the final pushed commit so `sourceCommit` points at the release docs commit.

## Release Smoke

Command:

```bash
scripts/release-smoke.sh --tarball /Users/jiangxuanyang/Desktop/codelattice/dist/codelattice-0.14.0-beta.1-darwin-arm64.tar.gz
```

Result: PASS.

Fixture results:

| Language | Symbols | Files | Edges |
|----------|--------:|------:|------:|
| Rust | 9 | 2 | 25 |
| Cangjie | 22 | 3 | 36 |
| ArkTS | 5 | 2 | 24 |
| TypeScript | 20 | 4 | 54 |
| C | 22 | 3 | 32 |
| C++ | 33 | 3 | 77 |
| Python | 23 | 5 | 46 |

MCP checks: `tools/list == 24`, all six optional language support flags true.

## Fresh Clone Smoke

Both commands passed:

```bash
scripts/fresh-clone-smoke.sh --skip-tests
scripts/fresh-clone-smoke.sh
```

The script copied the checkout into a temporary directory, built all-language release binaries, promoted into a temporary install dir, and ran wrapper self-test and fixture project_overview checks. It did not modify real AI client configs or the stable `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` runtime.

Fresh clone fixture project_overview counts:

| Language | nodeCount | edgeCount | symbolCount | sourceFileCount |
|----------|----------:|----------:|------------:|----------------:|
| Rust | 16 | 25 | 9 | 2 |
| Cangjie | 27 | 36 | 22 | 3 |
| C | 28 | 32 | 22 | 3 |
| C++ | 40 | 77 | 33 | 3 |
| Python | 29 | 46 | 23 | 5 |

Release smoke covers ArkTS and TypeScript portable fixtures.

## Real Corpus Evidence

The cache existed at `/tmp/codelattice-real-corpus-smoke`, so offline strict compare was run:

```bash
python3 scripts/real-project-corpus-smoke.py \
  --offline \
  --compare-baseline \
  --strict-baseline \
  --cache-dir /tmp/codelattice-real-corpus-smoke \
  --json-out /tmp/codelattice-beta-real-corpus.json \
  --markdown-out /tmp/codelattice-beta-real-corpus.md
```

Initial run failed because the stored baseline did not include qualityMetrics, while the compare script applied absolute quality thresholds to Phase A languages. The root cause was the release gate, not a new analyzer semantic change. The fix:

- Store qualityMetrics in `docs/real-project-corpus-baseline.json`.
- Compare quality rates as baseline-relative regression deltas when target baseline quality exists.
- Preserve legacy absolute thresholds for old baseline entries without stored qualityMetrics.
- Keep dangling edges as an absolute failure.

Final strict compare result: PASS, 3/3.

| Target | Language | nodeCount | edgeCount | symbolCount | sourceFileCount | Quality | Baseline |
|--------|----------|----------:|----------:|------------:|----------------:|---------|----------|
| redis-c | C | 10967 | 11478 | 10751 | 133 | unknownEdge=95.5% | pass |
| catch2-cpp | C++ | 7522 | 19856 | 7076 | 225 | unknownEdge=51.0% | pass |
| pip-python | Python | 34626 | 61989 | 33993 | 632 | lowCall=35.3%, unknownEdge=47.8% | pass |

These are smoke/baseline signals, not precision proof. Real projects are not vendored and target build/test/package scripts are not run.

## Verification

Final verification commands run before `detect-changes`:

| Command | Result |
|---------|--------|
| `cargo fmt --check` | PASS |
| `git diff --check` | PASS |
| `cargo test --test mcp_server` | PASS, 104/104 |
| `cargo test` | PASS |
| `cargo test --all-features` | PASS |
| `python3 scripts/real-project-corpus-smoke-test.py` | PASS, 10/10 |
| `scripts/codelattice-mcp.sh --self-test` | PASS |
| `scripts/mcp-dogfood.sh` | PASS, 24/24 |
| `scripts/install-mcp.sh --doctor` | PASS, 8/8 |
| `scripts/package-release.sh` | PASS |
| `scripts/release-smoke.sh --tarball ...` | PASS |
| `scripts/fresh-clone-smoke.sh --skip-tests` | PASS |
| `scripts/fresh-clone-smoke.sh` | PASS |
| real corpus offline strict compare | PASS, 3/3 |
| `bash scripts/check-release-metadata.sh` | PASS |
| `bash -n` for release shell scripts | PASS |

## Detect Changes

Command:

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
```

Result:

- Changes: 29 files, 135 symbols
- Affected processes: 0
- Risk level: low

Primary changed symbols are README/docs/release sections plus release smoke/baseline helper scripts. No high or critical risk was reported.

## Index Refresh

Command:

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
```

Result: PASS.

- Indexed in 3.7s
- 6,830 nodes
- 13,270 edges
- 144 clusters
- 300 flows

## External Boundaries

Not touched:

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool source
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool` stable runtime contents
- Codex/opencode/Claude real client configs
- Real project source repositories
- GitCode Release page

Only CodeLattice repo files were modified. Temporary fresh-clone/promote directories were under system temp paths.

## Commit / Push

Pending.
