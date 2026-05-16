# CodeLattice Release Packaging

CodeLattice release packaging is intentionally local and scriptable. It does not publish assets, edit AI client configuration, or promote into a user's stable runtime directory.

The current beta target is `v0.14.0-beta.1` with a `darwin-arm64` tarball and checksum. Multi-platform artifacts are planned next.

## Install a Published Release

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.14.0-beta.1 \
  --install-dir "$CODELATTICE_TOOL_DIR"
```

The installer verifies the `.sha256` checksum, installs the stable runtime wrapper, and runs `codelattice-mcp.sh --self-test`.

See [release-install.md](release-install.md) for options and safety behavior.

## Build a Release Artifact

```bash
bash scripts/package-release.sh
```

Default output:

```text
dist/codelattice-<version>-<platform>.tar.gz
dist/codelattice-<version>-<platform>.tar.gz.sha256
```

Options:

```bash
bash scripts/package-release.sh --version 0.14.0-beta.1
bash scripts/package-release.sh --platform darwin-arm64
bash scripts/package-release.sh --dist-dir /tmp/codelattice-dist
bash scripts/package-release.sh --skip-build
bash scripts/package-release.sh --keep-temp
```

## Artifact Layout

```text
codelattice-<version>-<platform>/
  bin/
    codelattice
    gitnexus-rust-core-cli
  codelattice-mcp.sh
  manifest.json
  README.md
  CHANGELOG.md
  LICENSE
  scripts/
    linux-source-build-smoke.sh
  docs/
    getting-started.md
    release-install.md
    release-versioning.md
    release-packaging.md
    release/
      upgrade.md
      smoke-matrix.md
    platforms/
      linux-openeuler.md
    architecture/
      mcp-local-client-setup.md
      mcp-v0-contract.md
  fixtures/
    rust/portable-smoke/
    cangjie/portable-smoke/
    arkts/portable-smoke/
    typescript/portable-smoke/
    c/portable-smoke/
    cpp/portable-smoke/
    python/portable-smoke/
```

`bin/codelattice` is the primary public binary. `bin/gitnexus-rust-core-cli` is included as a compatibility binary for older scripts.

## Version and Changelog

The product release version comes from Cargo `workspace.package.version`. MCP `serverVersion` is a separate sidecar tool/profile version and is recorded under the manifest `profile` block.

Before building a release artifact, validate release metadata:

```bash
bash scripts/check-release-metadata.sh
```

The release tarball includes `CHANGELOG.md` and `docs/release-versioning.md` so external users can inspect the release rules without the development checkout.

## Manifest

`manifest.json` records:

- package version and platform
- source remote and source commit when available
- relative artifact paths
- binary SHA-256 checksums
- build features
- MCP profile: server version, Cangjie/ArkTS/TypeScript/C/C++/Python support, tool count

The manifest avoids requiring the original development checkout.

## Smoke a Release Artifact

```bash
bash scripts/release-smoke.sh
```

The smoke script uses the newest `dist/codelattice-*.tar.gz` unless a tarball is specified:

```bash
bash scripts/release-smoke.sh --tarball dist/codelattice-0.14.0-beta.1-darwin-arm64.tar.gz
```

It verifies:

- `.sha256` checksum when present
- top-level release directory
- executable `bin/codelattice`
- executable compatibility binary
- executable `codelattice-mcp.sh`
- packaged `CHANGELOG.md`
- packaged `docs/release-install.md`
- packaged `docs/release-versioning.md`
- wrapper `--self-test`
- MCP `tools/list >= 24`
- Rust portable fixture analyze with nonzero symbols/files/edges
- Cangjie portable fixture analyze
- ArkTS portable fixture analyze
- TypeScript portable fixture analyze
- C portable fixture analyze
- C++ portable fixture analyze
- Python portable fixture analyze

The smoke unpacks into `/tmp` and cleans up by default. Use `--keep-temp` for debugging.

## Safety Guarantees

The packaging and smoke scripts do not:

- write Codex, opencode, Claude, or shell configuration
- promote into `$HOME/Desktop/CodeLattice-Tool`
- modify source projects
- require a live Cangjie repository
- require WebUI

`scripts/install-release.sh` follows the same client-config rule: it installs only the stable runtime files and prints the wrapper path for the user to configure separately.

## Recommended Release Checklist

Before publishing an artifact:

```bash
cargo fmt --check
git diff --check
bash scripts/check-release-metadata.sh
cargo test
cargo test --all-features
bash scripts/package-release.sh
bash scripts/release-smoke.sh
bash scripts/install-release.sh --dry-run --version v0.14.0-beta.1 --platform darwin-arm64
bash scripts/fresh-clone-smoke.sh --skip-tests
```
