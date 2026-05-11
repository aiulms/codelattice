# CodeLattice Release Readiness Pack Closure（2026-05-11）

## Summary

本轮完成非 WebUI 产品化闭环：

- `codelattice` public binary alias
- release tarball packaging
- release tarball smoke
- stable MCP promote/install scripts 优先使用 `codelattice`
- active smoke scripts 适配多 binary Cargo package
- public getting-started / release-packaging docs

不包含 WebUI、HTTP daemon、crate publish、GitCode release asset 发布或分析语义改动。

## Implementation

### Binary alias

- `crates/cli/src/main.rs` 移为 `crates/cli/src/lib.rs`，入口改为 `pub fn run()`。
- 新增两个薄 binary wrapper：
  - `crates/cli/src/bin/codelattice.rs`
  - `crates/cli/src/bin/gitnexus-rust-core-cli.rs`
- `crates/cli/Cargo.toml` 显式声明两个 `[[bin]]`。
- Clap public command name 改为 `codelattice`，`--version` 输出 `codelattice 0.1.0`。
- 旧 binary `gitnexus-rust-core-cli` 继续保留兼容。

TDD evidence：

- RED：`Command::cargo_bin("codelattice")` failed with `CARGO_BIN_EXE_codelattice is unset`。
- GREEN：`cargo test --test productization_commands codelattice_` passed 2/2。

### Release packaging

新增：

- `scripts/package-release.sh`
- `scripts/release-smoke.sh`

Default artifact：

```text
dist/codelattice-0.1.0-darwin-arm64.tar.gz
dist/codelattice-0.1.0-darwin-arm64.tar.gz.sha256
```

Tarball layout：

```text
codelattice-0.1.0-darwin-arm64/
  bin/codelattice
  bin/gitnexus-rust-core-cli
  codelattice-mcp.sh
  manifest.json
  README.md
  LICENSE
  docs/getting-started.md
  docs/release-packaging.md
  docs/architecture/mcp-local-client-setup.md
  docs/architecture/mcp-v0-contract.md
  fixtures/rust/portable-smoke/
  fixtures/cangjie/portable-smoke/
```

`release-smoke.sh` validates checksum, unpack, binary versions, wrapper `--self-test`, MCP `tools/list >= 21`, Rust fixture analyze, and Cangjie fixture analyze when supported.

### Script migration

Updated active scripts to prefer `codelattice`:

- `scripts/build.sh`
- `scripts/install-mcp.sh`
- `scripts/promote-to-local-tool.sh`
- `scripts/codelattice-mcp.sh`
- `scripts/mcp-dogfood.sh`
- `scripts/mcp-real-client-dry-run.sh`
- `scripts/mcp-cache-smoke.sh`

Updated Cargo run disambiguation after multiple binary targets:

- `scripts/smoke.sh`
- `scripts/verify-bridge.sh`
- `scripts/alpha-trial-smoke.sh`

Added `/dist` to `.gitignore`; generated release artifacts are local verification outputs, not committed.

### Docs

Added:

- `docs/getting-started.md`
- `docs/release-packaging.md`

Updated:

- `README.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/architecture/consumer-contract.md`
- `docs/smoke-targets-config.md`
- `docs/plans/README.md`

README now presents `codelattice` as the primary command and documents `gitnexus-rust-core-cli` as compatibility only.

## Verification

Passed:

- `cargo fmt --check`
- `git diff --check`
- `cargo test --test productization_commands` — 13/13
- `cargo test`
- `cargo test --features tree-sitter-cangjie`
- `bash scripts/install-mcp.sh --doctor` — 8/8
- `bash scripts/codelattice-mcp.sh --self-test`
- `bash scripts/package-release.sh`
- `bash scripts/release-smoke.sh`
- `bash scripts/fresh-clone-smoke.sh --skip-tests`
- `bash scripts/smoke.sh --quick` — 8 PASS / 0 FAIL / 1 SKIP
- `bash scripts/verify-bridge.sh` — 4/4
- `bash scripts/mcp-cache-smoke.sh` — 4/4
- `bash scripts/mcp-dogfood.sh` — 22/22
- `bash scripts/mcp-real-client-dry-run.sh` — 10/10
- `bash scripts/mcp-local-client-smoke.sh` — 9 PASS / 0 FAIL / 1 optional SKIP
- `bash -n scripts/*.sh`

Release smoke highlight：

- checksum OK
- `bin/codelattice --version` → `codelattice 0.1.0`
- `bin/gitnexus-rust-core-cli --version` → `codelattice 0.1.0`
- wrapper self-test PASS
- tools/list = 21
- Rust fixture: symbols=9, files=2, edges=25
- Cangjie fixture: symbols=22, files=3, edges=36

Fresh clone smoke highlight：

- temporary promote uses `bin/codelattice`
- wrapper self-test PASS
- tools/list = 21
- Rust project_overview nonzero
- Cangjie project_overview nonzero

## GitNexus Detect Changes

Command：

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
```

Unstaged/all before index refresh initially reported LOW for docs/script-heavy changes. After index refresh and staging, the CLI entrypoint refactor is visible as `run`, so the final staged result is：

- Changes: 28 files, 94 symbols
- Affected processes: 9
- Risk level: HIGH

Affected flows are all `Run → ...` flows from the CLI entrypoint extraction (`main.rs` → `lib.rs::run`). This is expected for the binary alias refactor. The risk is covered by:

- `cargo test`
- `cargo test --features tree-sitter-cangjie`
- productization command tests
- release tarball smoke
- fresh clone smoke
- MCP dogfood / real-client / local-client / cache smokes

## Boundaries

This round did not:

- implement WebUI
- publish release assets
- write real Codex/opencode/Claude config
- promote to the user's stable runtime except temporary `/tmp` fresh-clone smoke runtime
- modify GitNexus-RC runtime/schema/WebUI
- modify GitNexus-RC-Tool
- modify live Cangjie repo or open-nwe
- change Rust/Cangjie analysis semantics

## Follow-ups

- Decide release versioning policy and changelog format.
- Add CI release job when repository hosting supports release assets.
- Plan Cargo package rename from `gitnexus-rust-core-cli` to `codelattice` with compatibility window.
- Add external beta task package using release tarball instead of source checkout.
