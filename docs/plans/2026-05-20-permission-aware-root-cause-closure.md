# Permission-aware Root Cause Evidence Pack — Closure

Date: 2026-05-20
Status: Completed

## Summary

- Added permission metadata to every MCP `tools/list` entry:
  - standard `annotations` (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`);
  - `x-codelattice-permissionProfile` with source-write, project-code-execution, network, cache-write, temp-artifact, and confirmation fields.
- Added `codelattice_root_cause_assistant` as the 51st MCP tool.
- Added `codelattice_workflow mode=root_cause` routing to the root-cause assistant.
- Updated docs and smoke scripts for the 49-tool profile.
- Fixed `install-mcp.sh --doctor` Cangjie smoke to use `CODELATTICE_MCP_TOOLSET=full`; the previous failure was caused by default AI toolset hiding low-level `codelattice_symbol_search`, not by Cangjie analysis regression.

The new root-cause assistant remains advisory and read-only. It does not execute project code, call local HTTP endpoints, operate browsers, install probes, or modify source files. It only gives the AI a capability-aware evidence plan and structured root-cause hypotheses.

## Verification

- TDD RED observed for permission annotations: `cargo test --test mcp_server mcp_tools_list_permission -- --nocapture` failed before implementation because tool annotations were missing.
- TDD RED observed for root-cause assistant: `cargo test --test mcp_server mcp_root_cause -- --nocapture` failed before implementation because the tool did not exist.
- `cargo fmt --check`: PASS.
- `git diff --check`: PASS.
- `cargo test --test mcp_server mcp_tools_list_returns_forty_nine_tools -- --nocapture`: PASS.
- `cargo test --test mcp_server mcp_default_toolset_is_ai_friendly -- --nocapture`: PASS.
- `cargo test --test mcp_server mcp_tools_list_permission -- --nocapture`: PASS.
- `cargo test --test mcp_server mcp_root_cause -- --nocapture`: PASS.
- `cargo test --test mcp_server mcp_workflow_root_cause -- --nocapture`: PASS.
- `cargo test --test mcp_server`: PASS, 139 tests.
- `cargo test`: PASS.
- `cargo test --all-features`: PASS.
- `bash scripts/codelattice-mcp.sh --self-test`: PASS, 49 tools, all language support flags true.
- `bash scripts/mcp-dogfood.sh`: PASS, 49/49 checks including `codelattice_root_cause_assistant`.
- `python3 scripts/real-project-corpus-smoke-test.py`: PASS, 10 tests.
- `bash scripts/install-mcp.sh --doctor`: PASS, 8/8 checks after full-toolset doctor fix.
- `bash scripts/codelattice-precommit-check.sh`: PASS.

## Detect Changes Review

- Pre-edit native detect-changes reported no changed files or changed symbols. The current conservative risk level was `high`, with no concrete diff-driven risk.
- Final native precommit detect-changes reported `critical` risk:
  - tracked files: 19;
  - untracked files: 4;
  - changed symbols: 17;
  - unknown hunks: 57;
  - affected projects: 16;
  - cross-project risk: critical.
- Interpretation: expected conservative broad-surface rating because this pack intentionally touches the MCP server, MCP tests, smoke/installer scripts, README, CHANGELOG, MCP contract docs, and plan docs. Verification above passed; no runtime/project-code execution was introduced.

## Boundaries

- Did not modify GitNexus-RC runtime/schema/WebUI.
- Did not modify GitNexus-RC-Tool implementation.
- Did not modify `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Did not modify Codex/opencode/Claude config.
- Did not modify real project source trees.
- Did not execute target project build/test/package scripts.
