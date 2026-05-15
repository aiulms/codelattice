# CodeLattice Upgrade Guide

## From Source (Clone and Build)

1. Pull the latest changes:
   ```bash
   cd /path/to/codelattice
   git pull gitcode master
   ```

2. Rebuild:
   ```bash
   cargo build --release
   ```

3. Re-promote the runtime:
   ```bash
   bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
   "$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
   ```

## From Release Tarball

1. Download the new tarball and verify:
   ```bash
   shasum -a 256 -c codelattice-<version>-<platform>.tar.gz.sha256
   ```

2. Install into the tool directory:
   ```bash
   bash scripts/install-release.sh \
     --version v<new-version> \
     --install-dir "$CODELATTICE_TOOL_DIR"
   ```

3. Verify:
   ```bash
   "$CODELATTICE_TOOL_DIR/codelattice-mcp.sh" --self-test
   ```

## Rollback

If the new version causes issues:

1. Re-install the previous version tarball:
   ```bash
   bash scripts/install-release.sh \
     --version v<previous-version> \
     --install-dir "$CODELATTICE_TOOL_DIR"
   ```

2. Or rebuild from a known-good commit:
   ```bash
   cd /path/to/codelattice
   git checkout <previous-commit>
   cargo build --release
   bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
   ```

## Cache Cleanup

To clear persistent cache after an upgrade:

```bash
# Via MCP tool (if running)
# Call codelattice_cache_clear with layer="both"

# Or manually
rm -rf "$CODELATTICE_CACHE_DIR"/cl-cache-*.json
```

Memory cache clears automatically when the MCP server process restarts.

## Breaking Changes

Breaking changes are documented in `CHANGELOG.md` under the `Changed`, `Removed`, and `Compatibility` categories. During the external beta period, minor versions may include breaking MCP output changes — always check the changelog before upgrading.

The MCP `serverVersion` in the `initialize` response indicates the sidecar contract version. If it differs from your expected version, check the changelog for contract changes.

## Dry-Run Verification

To verify an upgrade without affecting your current runtime:

```bash
# Build in a temp directory
tmp_dir="$(mktemp -d)"
git clone --depth 1 https://gitcode.com/aiulms/codelattice.git "$tmp_dir"
cd "$tmp_dir"
cargo build --release

# Test without installing
target/release/codelattice --version
bash scripts/codelattice-mcp.sh --self-test

# Only promote when satisfied
bash scripts/promote-to-local-tool.sh --install-dir "$CODELATTICE_TOOL_DIR"
```
