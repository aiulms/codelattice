# CodeLattice Release Versioning + Changelog Preflight

Date: 2026-05-11

## Goal

Define the release versioning and changelog rules that make CodeLattice artifacts understandable outside the original development machine.

This pack is deliberately small: it adds release policy, changelog structure, an executable metadata check, and package/smoke wiring so release artifacts carry those rules.

## Scope

- Add a repository-level `CHANGELOG.md`.
- Add `docs/release-versioning.md`.
- Add `scripts/check-release-metadata.sh`.
- Include changelog and versioning docs in release packaging.
- Teach release smoke to verify those files are present.
- Cross-link the new policy from README and release packaging docs.

## Non-goals

- No WebUI.
- No Cargo package or binary rename.
- No product version bump in this pass.
- No publish/upload automation.
- No runtime schema or MCP tool contract change.
- No changes outside the CodeLattice repository.

## Version Policy Decision

`workspace.package.version` remains the source of truth for CodeLattice product releases. It controls `codelattice --version` and the default release tarball name.

MCP `serverVersion` remains a separate runtime/tool-profile version. It records the MCP sidecar capability profile and must not be treated as the public product version.

## Risk

Risk is low. Changes are docs and shell packaging checks. GitNexus impact on `scripts/package-release.sh` and `scripts/release-smoke.sh` reported LOW risk with no upstream dependants.

## Stop-line

If a verification failure points at Rust analysis semantics, Cangjie parsing, or MCP schema behavior, stop and do not fold that fix into this release-policy pack.
