# Linux and openEuler Source Build Guide

This page describes the current Linux / openEuler compatibility position for
CodeLattice.

CodeLattice is primarily language-oriented, not operating-system-oriented. It
analyzes source trees for Rust, Cangjie, ArkTS / HarmonyOS, and TypeScript. The
runtime is a Rust CLI plus shell wrappers.

## Current Status

| Area | Status |
|------|--------|
| Linux source build | Expected compatible, source-build path documented |
| openEuler source build | Expected compatible, not yet certified by a native openEuler smoke |
| Published binary artifacts | macOS Apple Silicon only for `v0.13.0-beta.1` |
| HarmonyOS NEXT source projects | Supported through ArkTS, TypeScript, and Cangjie analysis |
| Running CodeLattice on HarmonyOS NEXT | Not certified |

Do not describe openEuler as certified until a native openEuler VM, container,
or machine has run the smoke commands in this document.

## Source Support vs Runtime Support

CodeLattice support has two separate meanings:

- **Source analysis support**: whether CodeLattice can analyze a project written
  in a supported language. This is already available for Rust, Cangjie, ArkTS,
  and TypeScript.
- **Runtime platform support**: whether the CodeLattice binary and shell scripts
  have been built and smoked on a given OS / architecture.

For openEuler, Rust projects should be analyzable once CodeLattice builds. C,
C++, RPM spec files, systemd units, and shell-only openEuler projects are not
yet language adapters.

## Prerequisites

Install these tools with the platform package manager or a standard Rust
toolchain installer:

- `git`
- `bash`
- `cargo` / `rustc`
- a C toolchain if required by Rust crates on the target platform
- `find`, `sed`, `mktemp`, `tar`
- `python3` for MCP smoke helpers
- `sha256sum` or `shasum` for release checksum verification

Package names vary between Linux distributions. The project does not require
`npm`, `pnpm`, `yarn`, `tsc`, `vite`, or project build scripts for analysis.

## Quick Source Build

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice
cargo build --release -p gitnexus-rust-core-cli --bins
target/release/codelattice --version
```

To build all current language adapters:

```bash
cargo build --release -p gitnexus-rust-core-cli --bins \
  --features tree-sitter-cangjie,tree-sitter-arkts,tree-sitter-typescript
```

## Local Source Build Smoke

Run the portable preflight script:

```bash
bash scripts/linux-source-build-smoke.sh
```

To include all language features:

```bash
bash scripts/linux-source-build-smoke.sh --all-language-features
```

The smoke script uses a temporary `CARGO_TARGET_DIR` by default, so it does not
write build artifacts into the repository checkout.

## MCP Runtime from Source

Build and promote a stable MCP wrapper without editing AI client configs:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --build
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

Print client configuration templates only:

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

## Expected Verification

A Linux / openEuler source-build report should include:

```bash
cargo fmt --check
cargo test --test mcp_server
cargo test
bash scripts/linux-source-build-smoke.sh --all-language-features
bash scripts/codelattice-mcp.sh --self-test
```

Optional release path checks:

```bash
bash scripts/package-release.sh --platform linux-$(uname -m)
bash scripts/release-smoke.sh --tarball dist/codelattice-*-linux-*.tar.gz
```

## Known Caveats

- `v0.13.0-beta.1` publishes a macOS Apple Silicon tarball. Linux users should
  use source build until Linux release artifacts are published.
- Shell tools can differ between GNU/Linux and macOS. The project scripts avoid
  relying on macOS-only behavior where possible, but openEuler still needs a
  native smoke before it can be called certified.
- `sha256sum` is common on Linux; `shasum -a 256` is common on macOS. Release
  scripts should accept either where checksum verification is needed.
- Cangjie, ArkTS, and TypeScript support are static analysis adapters. They do
  not compile projects, install dependencies, or replace IDE / language server
  behavior.

## Certification Labels

Use these labels consistently:

- **Expected compatible**: source build path should work but has not completed a
  native platform smoke.
- **Smoke passed**: build and smoke commands completed on the platform.
- **Certified**: smoke passed on the release gate for a named OS, architecture,
  version, and CodeLattice version.

Current openEuler status: **expected compatible**.
