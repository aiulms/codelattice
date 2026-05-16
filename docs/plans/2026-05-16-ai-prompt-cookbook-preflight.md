# AI Prompt Cookbook Preflight

Date: 2026-05-16

## Goal

Turn CodeLattice's MCP tool set and workflow presets into copyable prompt
recipes for AI assistants and external beta users.

This is a documentation/productization pack. It does not add MCP tools, change
analysis behavior, or run project code.

## Scope

Write set:

- `docs/guides/README.md`
- `docs/guides/ai-prompt-cookbook.md`
- `docs/guides/workflow-presets.md`
- `README.md`
- `CHANGELOG.md`
- `docs/plans/README.md`
- `docs/plans/2026-05-16-ai-prompt-cookbook-closure.md`

Forbidden set:

- No CodeLattice runtime or analyzer changes.
- No GitNexus-RC or GitNexus-RC-Tool changes.
- No `CodeLattice-Tool` promote.
- No AI client configuration writes.
- No real project source changes.
- No WebUI work.

## Design

Create two external-facing guides:

- `ai-prompt-cookbook.md`: copyable prompts for common AI coding workflows.
- `workflow-presets.md`: a compact map from scenario to MCP tool sequence,
  stop-lines, and fields to inspect.

The cookbook should teach users how to ask for:

- onboarding a new codebase;
- before-edit risk analysis;
- after-edit review;
- dead-code candidate review;
- breaking-change review;
- docs/tests/config consistency checks;
- release checks;
- legacy cleanup.

Every prompt must preserve CodeLattice's safety language:

- static analysis is not runtime proof;
- dead code candidates are not deletion proof;
- public API and framework entries need extra caution;
- CodeLattice does not execute project code.

## Verification Plan

- `cargo fmt --check`
- `git diff --check`
- `cargo test`
- `python3 scripts/real-project-corpus-smoke-test.py`
- `scripts/codelattice-mcp.sh --self-test`
- `scripts/mcp-dogfood.sh`
- `gitnexus detect-changes`

Because this pack is docs-only, full feature-specific cargo suites are not
expected to change. If any verification fails, record the exact command and
error before proceeding.
