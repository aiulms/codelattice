# CodeLattice Release Install

This guide installs a published CodeLattice release tarball without building from source.

The installer downloads the release artifact, verifies the `.sha256` checksum, installs the stable MCP wrapper, and runs `codelattice-mcp.sh --self-test`.

It does not write Codex, opencode, Claude, or shell configuration files.

## Current Published Binary

`v0.13.0-beta.2` publishes a macOS Apple Silicon artifact:

```text
codelattice-0.13.0-beta.2-darwin-arm64.tar.gz
```

Linux users can already clone and build from source. Multi-platform release artifacts are the next packaging step. See the [Linux / openEuler source build guide](platforms/linux-openeuler.md) for prerequisites and smoke commands.

## Install

```bash
export CODELATTICE_TOOL_DIR="$HOME/.local/share/codelattice-tool"
tmp_dir="$(mktemp -d /tmp/codelattice-install-XXXXXX)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
bash "$tmp_dir/scripts/install-release.sh" \
  --version v0.13.0-beta.2 \
  --install-dir "$CODELATTICE_TOOL_DIR"
```

Verify:

```bash
"$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
```

Use this wrapper path in MCP clients:

```text
$CODELATTICE_TOOL_DIR/codelattice-mcp.sh
```

## Options

```bash
bash scripts/install-release.sh --version v0.13.0-beta.2
bash scripts/install-release.sh --platform darwin-arm64
bash scripts/install-release.sh --install-dir "$HOME/.local/share/codelattice-tool"
bash scripts/install-release.sh --dry-run
bash scripts/install-release.sh --keep-temp
```

`--force` allows installing into a non-empty directory that does not already look like a CodeLattice runtime. Use it only for a directory dedicated to CodeLattice.

## Safety

The installer:

- verifies the release checksum before unpacking
- validates the unpacked wrapper before installing
- validates the installed wrapper after copying
- refuses to overwrite a non-empty non-CodeLattice directory unless `--force` is provided
- does not modify AI client configuration
- does not require a development checkout

## Troubleshooting

If download fails with 404, the selected release probably does not provide that platform artifact yet. For now, non-`darwin-arm64` users should clone and build:

```bash
git clone https://gitcode.com/aiulms/codelattice.git
cd codelattice
bash scripts/install-mcp.sh --build
```

For a fuller platform preflight, run:

```bash
bash scripts/linux-source-build-smoke.sh --all-language-features
```
