# Findings: P0 Cache Optimization Closure

## Requirements

- Plan and implement the improvements identified in the P0 completion review.
- Preserve IMPORTS/CALLS evidence, confidence, reason, endpoint integrity, and deterministic ordering.
- Resolve the CLI `analysisTrace` compatibility issue.
- Repair Phase A/Phase C governance records.
- Re-run required native verification without modifying live repositories.

## Verified Starting Evidence

- `extract_and_resolve_imports` and `extract_and_resolve_calls` parallelize independent per-file work and sort after merge.
- Current Rust graph fixtures and three local repeated analyses are deterministic after removing volatile timing fields.
- Full `cargo test`, `cargo fmt --check`, and `git diff --check` passed before this closure slice.
- Native precommit smoke passed 17/17, while the complete workspace review reported critical risk because 22 unrelated/mixed worktree files were present.
- Non-Rust full and compact CLI outputs currently serialize `"analysisTrace": null`.
- The Phase A card explicitly forbids Phase C and says it needs a new execution card, yet appends Phase C execution in the same file.
- The original strict MCP target is `<3s`; the recorded result is 3.1s and therefore near-target rather than strictly met.
- Pre-edit native impact is MEDIUM for all four production targets; no target reported high or critical risk.
- Rust deep-support self-check: this slice changes landed scheduling/output behavior only, existing fixtures cover IMPORTS/CALLS evidence and absence rules, no confidence/reason changes are allowed, and no MVP stop-line is crossed.

## Technical Decisions

| Decision | Rationale |
|---|---|
| Add productization tests before changing serialization | TDD proves the compatibility fix catches the current null-field behavior. |
| Omit `analysisTrace` when it is `None` in both full and compact profiles | Preserves non-Rust field sets while exposing real Rust trace data. |
| Reject `flat_map_iter` for this slice | Local A/B proxies showed no benefit and raw medians regressed, so the existing evidence-backed `flat_map` implementation is retained. |
| No confidence/reason or resolver changes | This slice is scheduling and contract closure only. |

## Performance Experiment

- A follow-up `flat_map_iter` scheduling experiment was rejected. On the allowed local proxies it produced no measurable benefit and raw medians regressed; the two production lines were restored to `flat_map`.
- The existing per-file Rayon implementation remains the evidence-backed optimization. No additional scheduling change will be retained without a larger, controlled benchmark.

## Closure

- Phase A is closed as an evidence-driven no-op and hands off to the independent Phase C execution card.
- The strict `<3s` milestone remains unmet by 0.1s in the recorded MCP sample; 3.1s is documented as near-target.
- All implementation gates passed. The remaining `critical` native risk is a mixed-worktree aggregate, while each P0 production symbol was MEDIUM in the pre-edit impact review.
- No stage, commit, push, live-repository modification, or production analysis was performed.

## Submission Audit

- Current branch is `master`; repository policy push target is `gitcode/master`.
- The worktree contains two coherent deliverables: (1) P0 cache/trace closure and (2) the earlier AI tool-surface consistency pack. They should be separate commits.
- `.cursor/` and `.omo/` are local editor/agent state and are explicitly excluded from staging.
- A secondary remote contains an embedded credential in its configured URL. It will not be used, printed again, or modified in this task; publication is restricted to `gitcode`.
- The AI-tooling diff is internally coherent: tool count 49→50, compile-time server version, guide/contract alignment, known-limitations inventory, recovery cookbook, facade-equivalence mapping, and its execution pack.
- The P0 diff is internally coherent: imports/calls per-file Rayon work, Rust CLI trace propagation, non-Rust omission contract, two productization tests, output contracts, changelog performance evidence, and Phase A/Phase C governance.
- Exact commit A paths: `README.md`, `crates/cli/src/mcp_server.rs`, the MCP/AI guide and limitation docs, `docs/plans/2026-08-01-ai-usage-optimization-pack.md`, plus only the AI Added/Fixed portion of `CHANGELOG.md`.
- Exact commit B paths: `crates/project-model/src/{imports,calls}.rs`, `crates/cli/src/{lib,unified_types}.rs`, `crates/cli/tests/productization_commands.rs`, both output contracts, Phase A/Phase C cards, plus only the Performance portion of `CHANGELOG.md`.
- Exact commit C paths: the three files under `docs/plans/2026-08-02-p0-cache-optimization-closure/`.
- The only excluded untracked paths are `.cursor/mcp.json` and four `.omo/run-continuation/*.json` files.
- Commit A: `0e207a8 fix(mcp): align AI tool surface contracts`.
- Commit B: `598069d perf(project-model): parallelize import and call resolution`.
- GitCode accepted both commits on `master` (`4e2dca7..598069d`) and its remote hooks passed.
- Immediately after the implementation push, the only remaining paths were the explicitly excluded `.cursor/` / `.omo/` state and this closure-record directory.

## Resources

- `AGENTS.md`
- `docs/plans/2026-08-01-mcp-cache-warm-phase-a-execution-card.md`
- `docs/plans/2026-08-01-ai-usage-optimization-pack.md`
- `docs/architecture/unified-output-contract.md`
- `docs/architecture/consumer-contract.md`
- `crates/cli/src/unified_types.rs`
- `crates/cli/src/lib.rs`
- `crates/project-model/src/imports.rs`
- `crates/project-model/src/calls.rs`
- `docs/plans/2026-08-02-mcp-cache-warm-phase-c-execution-card.md`
