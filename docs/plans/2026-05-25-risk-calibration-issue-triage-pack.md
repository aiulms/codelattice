# Risk Calibration + Issue Triage Pack

Date: 2026-05-25

## Goal

Make CodeLattice more useful for AI project diagnosis by improving risk prioritization and turning natural-language issue questions into one-call static triage summaries.

## Execution Card

Write set:

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `docs/mcp/ai-usage-guide.md`
- `CHANGELOG.md`

Forbidden set:

- Do not sync `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Do not modify live repositories such as `open-nwe`.
- Do not add a new MCP tool.
- Do not treat static analysis as runtime, reproduction, or coverage proof.

Stop line:

- Stop if default AI toolset changes from 6 facade tools or full toolset changes from 49 tools.
- Stop if `compact=true` payloads regain large evidence dumps.
- Stop if issue triage executes target project code or scripts.

## Design

1. Add calibrated risk metadata to project risk items:
   - Keep raw static score visible.
   - Add rank-relative calibrated level, percentile band, tie-break explanation, and rank guidance.
   - Ensure AI can choose between equal-looking `high` items by rank and drivers.
2. Improve `workflow ask` for issue-like questions:
   - Route natural-language symptoms to static project diagnosis in the same call when root is available.
   - Return a compact `triagePlan` with likely areas, read-first files, static hypotheses, and evidence gaps.
   - Preserve recommended next calls for deeper symbol/impact review.
3. Update AI usage docs with risk calibration and one-call issue triage guidance.

## Verification

- Add failing MCP regression tests first.
- Run targeted tests for project quick risk and ask locate_issue.
- Run `cargo fmt --check`, `git diff --check`, `cargo test --test mcp_server`.
- Run `scripts/codelattice-installed-acceptance.sh --dev-only`.
- Run native precommit before committing.
