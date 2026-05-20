# Permission-aware Root Cause Evidence Pack — Preflight

Date: 2026-05-20
Status: Approved by user direction

## Goal

Reduce unnecessary MCP approval prompts by publishing machine-readable permission metadata for every CodeLattice tool, and add an AI-facing root-cause evidence assistant that turns bug reports into static hypotheses plus the smallest useful evidence-capture plan.

## Non-goals

- Do not bypass the MCP client's own security model.
- Do not execute project code, tests, browsers, HTTP probes, or shell commands from the new root-cause tool.
- Do not write source files, install probes, start daemons, or persist watchers.
- Do not claim runtime proof, coverage proof, or guaranteed root cause.
- Do not add language semantics or parser capability.
- Do not modify GitNexus-RC, GitNexus-RC-Tool implementation, CodeLattice-Tool stable install, AI client config, or real project source trees.
- Do not package or publish a release.

## Design Direction

### Permission-aware MCP tools

- Add standard MCP `annotations` to `tools/list` entries.
- Add a CodeLattice-specific `x-codelattice-permissionProfile` object so AI clients can reason about what each tool may read/write/execute.
- Default all static analysis tools to `readOnlyHint=true`, `destructiveHint=false`, `openWorldHint=false`.
- Mark cache/export/smoke-like tools precisely:
  - cache clear/prewarm may write CodeLattice cache only;
  - export bridge may write `/tmp` artifacts only;
  - smoke may run CodeLattice smoke commands but still must not execute target project code.
- This reduces pointless prompts in clients that honor MCP annotations; clients may still prompt based on their own policy.

### Root Cause Evidence Loop

- Add `codelattice_root_cause_assistant` to the AI toolset.
- Input: `root`, `language`, issue/observed error/reproduction text, optional changed symbols/files, optional `availableCapabilities`, optional pasted `runtimeEvidence`.
- Output:
  - capability-aware permission summary;
  - static root-cause hypotheses from graph matches;
  - missing evidence;
  - one best next action;
  - probe options that an AI can apply only if it already has matching permissions;
  - privacy/safety cautions;
  - likely fix areas and next verification.
- The tool is advisory and read-only. It does not add probes itself.

## Write Set

- `crates/cli/src/mcp_server.rs`
- `crates/cli/tests/mcp_server.rs`
- `scripts/mcp-dogfood.sh`
- `scripts/codelattice-mcp.sh`
- `scripts/install-mcp.sh`
- `docs/architecture/mcp-v0-contract.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/guides/ai-prompt-cookbook.md`
- `README.md`
- `CHANGELOG.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-20-permission-aware-root-cause-preflight.md`
- `docs/plans/2026-05-20-permission-aware-root-cause-closure.md`
- `docs/superpowers/specs/2026-05-20-permission-aware-root-cause-design.md`
- `docs/superpowers/plans/2026-05-20-permission-aware-root-cause.md`

## Forbidden Set

- GitNexus-RC runtime/schema/WebUI
- GitNexus-RC-Tool implementation
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- Codex/opencode/Claude config
- Real project source trees
- Target project build/test/package scripts

## Stop-line

Stop and report if implementation requires actual runtime instrumentation, AI client config mutation, project-source writes from the MCP tool itself, network access, background agents/watchers, or a graph schema/runtime semantic change.

## Verification Plan

- TDD RED/GREEN for tool permission annotations.
- TDD RED/GREEN for `codelattice_root_cause_assistant` contract.
- TDD RED/GREEN for `codelattice_workflow mode=root_cause` routing.
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test mcp_server`
- `cargo test`
- `cargo test --all-features`
- `bash scripts/codelattice-mcp.sh --self-test`
- `bash scripts/mcp-dogfood.sh`
- `bash scripts/codelattice-precommit-check.sh`
- native detect-changes before commit

