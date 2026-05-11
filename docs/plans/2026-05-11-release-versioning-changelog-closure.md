# CodeLattice Release Versioning + Changelog Closure

Date: 2026-05-11

## Outcome

Release versioning and changelog rules are now explicit, documented, and checked by script.

## Changes

- Added repository `CHANGELOG.md` with `Unreleased` and `0.1.0` release-readiness baseline sections.
- Added `docs/release-versioning.md` defining the product version source, SemVer rules, changelog categories, release checklist, and compatibility-binary policy.
- Added `scripts/check-release-metadata.sh` to validate:
  - Cargo `workspace.package.version`
  - current `CHANGELOG.md` section
  - release policy presence
  - MCP `serverVersion` separation
  - package/smoke script wiring
- Updated release packaging to include `CHANGELOG.md` and `docs/release-versioning.md` in tarballs and manifest paths.
- Updated release smoke to fail if changelog or release policy is missing.
- Cross-linked release metadata checks from README, getting-started, and release packaging docs.

## Version Boundary

Product release version remains Cargo `workspace.package.version` (`0.1.0` in this pass).

MCP `serverVersion` remains separate and continues to describe the MCP sidecar tool/profile contract (`0.7.0` in current runtime checks).

## Verification

Passed:

- `bash -n scripts/*.sh`
- `bash scripts/check-release-metadata.sh`
- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands`
- `bash scripts/package-release.sh`
- `bash scripts/release-smoke.sh`
- `bash scripts/fresh-clone-smoke.sh --skip-tests`
- `cargo test`
- `cargo test --features tree-sitter-cangjie`
- `bash scripts/install-mcp.sh --doctor`
- `bash scripts/codelattice-mcp.sh --self-test`
- Tool detect-changes: 12 files, 39 symbols, 0 affected processes, LOW risk
- Tool index refresh: 5,160 nodes, 9,580 edges, 117 clusters, 281 flows

Notes:

- Existing Rust/Cangjie compiler warnings are unchanged and unrelated to this docs/scripts pack.
- No real AI client config was written.
- No stable runtime promotion outside temp/release smoke paths was performed.

## Boundaries Honored

- No GitNexus-RC runtime, schema, Tool, or WebUI changes.
- No live Cangjie/open-nwe repo changes.
- No Cargo package or binary rename.
- No release publication/upload automation.
