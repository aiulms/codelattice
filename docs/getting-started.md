# CodeLattice Getting Started

This guide walks through a fresh local setup without WebUI or hosted services.

## Requirements

- Rust toolchain with `cargo`
- Bash
- `python3` for smoke scripts and MCP response checks
- macOS or Linux shell environment

CodeLattice runs locally and does not upload source code.

## Clone and Build

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice

bash scripts/install-mcp.sh --build
```

The primary public binary is:

```bash
target/release/codelattice
```

The package still also builds `target/release/gitnexus-rust-core-cli` as a compatibility binary.

## Verify the Checkout

For a quick external-user path:

```bash
bash scripts/fresh-clone-smoke.sh --skip-tests
```

For a fuller local check:

```bash
cargo fmt --check
cargo test
cargo test --features tree-sitter-cangjie
bash scripts/install-mcp.sh --doctor
bash scripts/codelattice-mcp.sh --self-test
```

## Analyze a Project

Rust:

```bash
target/release/codelattice analyze \
  --root /path/to/rust/project \
  --language rust \
  --format json
```

Cangjie / 仓颉:

```bash
target/release/codelattice analyze \
  --root /path/to/cangjie/project \
  --language cangjie \
  --format json \
  --strict
```

Auto language detection:

```bash
target/release/codelattice analyze \
  --root /path/to/project \
  --language auto \
  --format json
```

## Install MCP Runtime

AI clients should point at a stable promoted runtime instead of the development checkout:

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

Print client config snippets:

```bash
bash scripts/install-mcp.sh --install-dir "$CODELATTICE_TOOL_DIR" --print-config
```

This command only prints templates. It does not write Codex, opencode, Claude, or shell config files.

## Package a Release Tarball

```bash
bash scripts/check-release-metadata.sh
bash scripts/package-release.sh
bash scripts/release-smoke.sh
```

The tarball contains:

- `bin/codelattice`
- `bin/gitnexus-rust-core-cli`
- `codelattice-mcp.sh`
- `manifest.json`
- `CHANGELOG.md`
- selected docs
- portable Rust/Cangjie fixtures for smoke

See [release-versioning.md](release-versioning.md) and [release-packaging.md](release-packaging.md) for release rules and artifact details.

## Troubleshooting

If Cangjie support is missing:

```bash
bash scripts/install-mcp.sh --build
```

If an MCP client cannot start the wrapper, use an absolute wrapper path from:

```bash
bash scripts/install-mcp.sh --print-config
```

If release smoke fails, keep the temp directory for inspection:

```bash
bash scripts/release-smoke.sh --keep-temp
```
