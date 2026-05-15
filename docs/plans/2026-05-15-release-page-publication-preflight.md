# Release Page Publication Preflight - 2026-05-15

## Goal

Prepare the external beta release package and a GitCode-ready release page for `v0.13.0-beta.1`.

## Scope

- Regenerate the `darwin-arm64` release tarball and checksum.
- Smoke-test the packaged artifact.
- Add a release page Markdown draft under `docs/release/`.
- Update release install docs and installer defaults from `v0.1.0` to `v0.13.0-beta.1`.
- Create and push the `v0.13.0-beta.1` tag if verification passes.

## Forbidden

- Do not modify analysis semantics or language adapters.
- Do not touch GitNexus-RC runtime, schema, or WebUI.
- Do not modify live external repositories.
- Do not publish a remote GitCode Release through a browser/API without credentials or a reliable CLI path.

## Verification

- `bash scripts/check-release-metadata.sh`
- `cargo fmt --check`
- `git diff --check`
- `cargo test`
- `bash scripts/package-release.sh`
- `bash scripts/release-smoke.sh --tarball dist/codelattice-0.13.0-beta.1-darwin-arm64.tar.gz`
- `bash scripts/install-release.sh --dry-run --version v0.13.0-beta.1 --platform darwin-arm64`
- `gitnexus detect_changes`
