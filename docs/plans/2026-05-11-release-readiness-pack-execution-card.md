# CodeLattice Release Readiness Pack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a non-WebUI release readiness loop for CodeLattice: binary alias, release tarball, tarball smoke, and public docs.

**Architecture:** Keep the existing CLI implementation as the single source of truth. Add `codelattice` as a Cargo binary alias pointing to the same `main.rs`; add release packaging scripts around the release binary and MCP wrapper without changing analysis semantics.

**Tech Stack:** Rust workspace/Cargo, Bash scripts, tar/shasum, existing MCP JSON-RPC stdio binary, existing fixtures.

---

## File Map

- `crates/cli/Cargo.toml` — declare `codelattice` and `gitnexus-rust-core-cli` binary outputs backed by `src/main.rs`.
- `crates/cli/tests/productization_commands.rs` — regression test proving `codelattice` alias runs the same CLI surface.
- `scripts/package-release.sh` — build/package release tarball into `dist/`.
- `scripts/release-smoke.sh` — unpack a release tarball into `/tmp` and validate binary/wrapper/MCP/fixtures.
- `scripts/build.sh`, `scripts/install-mcp.sh`, `scripts/promote-to-local-tool.sh`, `scripts/codelattice-mcp.sh` — prefer `codelattice` binary while retaining compatibility fallback.
- `scripts/smoke.sh`, `scripts/verify-bridge.sh`, `scripts/alpha-trial-smoke.sh`, `scripts/mcp-dogfood.sh`, `scripts/mcp-real-client-dry-run.sh`, `scripts/mcp-cache-smoke.sh` — disambiguate Cargo runs after adding multiple bin targets.
- `.gitignore` — keep generated `dist/` artifacts out of commits.
- `README.md` — show `codelattice` as the primary command, keep old name as compatibility fallback.
- `docs/getting-started.md` — external user setup walkthrough.
- `docs/release-packaging.md` — release artifact layout and smoke workflow.
- `docs/plans/2026-05-11-release-readiness-pack-closure.md` — final verification and commit record.

## Tasks

### Task 1: Binary Alias Regression

**Files:**
- Modify: `crates/cli/tests/productization_commands.rs`
- Modify: `crates/cli/Cargo.toml`

- [ ] Add a test `codelattice_binary_alias_runs_analyze` that uses `Command::cargo_bin("codelattice")` to analyze `fixtures/rust/portable-smoke`.
- [ ] Run `cargo test --test productization_commands codelattice_binary_alias_runs_analyze`; expected RED because `codelattice` binary does not exist yet.
- [ ] Add explicit `[[bin]]` entries for `codelattice` and `gitnexus-rust-core-cli`, both pointing to `src/main.rs`.
- [ ] Re-run the focused test; expected GREEN.

### Task 2: Release Smoke First

**Files:**
- Create: `scripts/release-smoke.sh`
- Create: `scripts/package-release.sh`

- [ ] Add `scripts/release-smoke.sh` that expects a tarball, unpacks it into `/tmp`, runs binary version/analyze, runs wrapper `--self-test`, checks tools/list count >= 21, and cleans up by default.
- [ ] Run `bash scripts/release-smoke.sh`; expected RED because no tarball exists and no package script exists.
- [ ] Add `scripts/package-release.sh` to build the release binary with Cangjie support and create `dist/codelattice-<version>-<platform>.tar.gz` plus `.sha256`.
- [ ] Run `bash scripts/package-release.sh --install-dir /tmp/codelattice-release-packaging-check` and then `bash scripts/release-smoke.sh --tarball <created-tarball>`; expected GREEN.

### Task 3: Public Docs

**Files:**
- Modify: `README.md`
- Create: `docs/getting-started.md`
- Create: `docs/release-packaging.md`
- Modify: `docs/plans/README.md`

- [ ] Update README Quick Start to prefer `codelattice` after build/package, with `gitnexus-rust-core-cli` documented as legacy compatibility.
- [ ] Add getting-started doc with clone/build/package/MCP client setup/analyze examples.
- [ ] Add release-packaging doc with artifact layout, checksum, smoke, and no-real-config-write policy.
- [ ] Update plans index with this pack.

### Task 4: Verification and Closure

**Files:**
- Create: `docs/plans/2026-05-11-release-readiness-pack-closure.md`

- [ ] Run `cargo fmt --check`.
- [ ] Run `git diff --check`.
- [ ] Run `cargo test --test productization_commands`.
- [ ] Run `cargo test`.
- [ ] Run `cargo test --features tree-sitter-cangjie`.
- [ ] Run `bash scripts/package-release.sh`.
- [ ] Run `bash scripts/release-smoke.sh`.
- [ ] Run `bash scripts/fresh-clone-smoke.sh --skip-tests`.
- [ ] Run GitNexus detect-changes.
- [ ] Refresh GitNexus codelattice index if changes are substantial.
- [ ] Write closure doc with final outputs, tarball path, test results, detect-changes risk, commit hash, and push status.
