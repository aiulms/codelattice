# ArkTS Parse-Once Extraction Pack

**Goal:** Extend the script-language parse-once optimization to ArkTS so an `.ets` file is parsed once for TypeScript-base symbols/imports/references and ArkTS component recovery.

**Architecture:** Reuse the TypeScript parse-once extractor from a public root-node helper, add an ArkTS combined extraction wrapper, and switch the CLI ArkTS analyzer to the wrapper. Preserve existing graph schema and ArkTS component semantics.

**Write Set:**
- `crates/typescript/src/extractors/mod.rs`
- `crates/arkts/src/lib.rs`
- `crates/arkts/src/extractors/component.rs`
- `crates/cli/src/lib.rs`
- `docs/plans/2026-05-31-arkts-parse-once-pack.md`

**Forbidden Set:**
- Do not modify live repos.
- Do not change MCP tool counts.
- Do not change graph node/edge schemas.
- Do not execute target project code.

**Stop-line:**
- If ArkTS portable-smoke node/edge counts change, stop and investigate before committing.

**Validation:**
- Red/green ArkTS unit tests for root-based component extraction and combined extraction.
- ArkTS CLI/MCP tests with `tree-sitter-arkts`.
- `cargo fmt --check`, `git diff --check`, and native precommit before commit.
