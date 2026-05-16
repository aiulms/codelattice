# AI Prompt Cookbook Closure

Date: 2026-05-16

## Summary

Added external-facing prompt and workflow guides for using CodeLattice's MCP
tool set. The pack is documentation-only and does not change analysis behavior.

## Delivered

- `docs/guides/README.md`: guide index.
- `docs/guides/ai-prompt-cookbook.md`: copyable prompts for common AI coding
  workflows.
- `docs/guides/workflow-presets.md`: scenario-to-tool mapping for the
  `codelattice_workflow_presets` MCP tool.
- README guide links and MCP workflow guidance.
- CHANGELOG entry for the AI prompt cookbook.
- docs/plans index entry.

## Boundaries

- No runtime or MCP implementation changes.
- No GitNexus-RC / GitNexus-RC-Tool changes.
- No stable `CodeLattice-Tool` promote.
- No AI client configuration writes.
- No real project source changes.

## Verification

- `cargo fmt --check`: PASS
- `git diff --check`: PASS
- `cargo test`: PASS
- `python3 scripts/real-project-corpus-smoke-test.py`: PASS
- `cargo build --release -p gitnexus-rust-core-cli --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript,tree-sitter-c,tree-sitter-cpp,tree-sitter-python --bins`: PASS
- `scripts/codelattice-mcp.sh --self-test`: PASS, 37 tools
- `scripts/mcp-dogfood.sh`: PASS, 37/37

`cargo test` was re-run after a targeted ArkTS disabled-feature check confirmed
the earlier transient failure was caused by stale/concurrent target state rather
than this docs-only change.
