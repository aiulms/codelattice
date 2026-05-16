# CodeLattice Release Beta Hardening Pack Preflight

Date: 2026-05-16

## Goal

把 CodeLattice 从多语言能力已经落地的状态，收口为可对外 beta 试用的发行包：

- 明确 beta 版本号、release notes、CHANGELOG 和 README 首页叙事。
- 确保 release tarball 默认包含 Rust / Cangjie / ArkTS / TypeScript / C / C++ / Python 的完整 language feature。
- 让外部用户能按最短路径完成下载、self-test、fixture analyze 和 MCP 配置。
- 用 release smoke、fresh clone smoke、MCP dogfood 和 real-project corpus baseline 给 beta 可用性留证。

## Non-goals

- 不新增语言能力，不扩分析语义。
- 不做 WebUI。
- 不把 CodeLattice 说成编译器、IDE、语言服务器或 GitNexus-RC 替代品。
- 不修改 GitNexus-RC runtime / adapter / schema / WebUI。
- 不修改 GitNexus-RC-Tool。
- 不修改真实项目源码。
- 不 promote 到 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`。
- 不修改 Codex / opencode / Claude 的真实配置。
- 不创建 git tag，不发布 GitCode Release 页面。
- 不执行目标项目 build / test / package scripts。

## Truth Gate

当前 HEAD:

```text
3594edf feat(c-cpp): refine include resolution with compile_commands.json
```

Stage 0 baseline 已重跑：

```text
cargo fmt --check                         PASS
git diff --check                          PASS
cargo test --test mcp_server              PASS (104/104)
cargo test                                PASS
cargo test --all-features                 PASS
python3 scripts/real-project-corpus-smoke-test.py PASS (8/8)
scripts/codelattice-mcp.sh --self-test    PASS (24 tools, all language flags true)
scripts/mcp-dogfood.sh                    PASS (24/24)
```

Pre-existing blocker found and fixed before this preflight: default `cargo test` compiled C/C++ graph integration tests without enabling `tree-sitter-c` / `tree-sitter-cpp`. The fix feature-gates only the extractor-backed graph tests; default compile_commands/include resolver tests remain enabled, and `cargo test --all-features` still runs the graph tests.

## Version Strategy

Current `workspace.package.version` is `0.13.0-beta.2`.

Recommended bump: `0.14.0-beta.1`.

Rationale:

- This pack graduates the accumulated v0.14-v0.17 work from `Unreleased` into an external beta identity.
- The release artifact now covers new full-language contents and real-project hardening across TypeScript, Python, C, and C++.
- This exceeds a patch-only packaging correction and should be represented as a new minor beta.

Only Cargo workspace version changes. Cargo package names, binary names, compatibility binary names, CLI command names, and MCP serverVersion stay on their existing contract unless a verification blocker requires a narrow fix.

No git tag is created in this task.

## Artifact Strategy

- Use `scripts/package-release.sh` to build the release artifact.
- Default build features must include:
  - `tree-sitter-cangjie`
  - `tree-sitter-arkts`
  - `tree-sitter-typescript`
  - `tree-sitter-c`
  - `tree-sitter-cpp`
  - `tree-sitter-python`
- Leave generated artifact under the existing script-controlled `dist/` / release-artifact location.
- Do not upload the artifact or edit external release pages.

## Release Smoke Scope

Required packaged fixtures:

- Rust: `fixtures/rust/portable-smoke`
- Cangjie: `fixtures/cangjie/portable-smoke`
- ArkTS: `fixtures/arkts/portable-smoke`
- TypeScript: `fixtures/typescript/portable-smoke`
- C: `fixtures/c/portable-smoke`
- C++: `fixtures/cpp/portable-smoke`
- Python: `fixtures/python/portable-smoke`

Required MCP profile checks:

- `cangjieSupport`
- `arktsSupport`
- `typescriptSupport`
- `cSupport`
- `cppSupport`
- `pythonSupport`
- `toolCount >= 24`

## Docs Update Scope

Write set:

- `Cargo.toml`
- `Cargo.lock` only if Cargo refreshes workspace package metadata
- `README.md`
- `CHANGELOG.md`
- `docs/release-versioning.md`
- `docs/release/smoke-matrix.md`
- `docs/release/0.14.0-beta.1-notes.md`
- `docs/release/real-corpus-baseline-report.md`
- `docs/architecture/mcp-local-client-setup.md`
- `docs/plans/2026-05-16-release-beta-hardening-preflight.md`
- `docs/plans/2026-05-16-release-beta-hardening-closure.md`
- `docs/plans/README.md`
- Release smoke / manifest scripts only if audit finds missing beta gate coverage.

Forbidden set:

- GitNexus-RC repository.
- GitNexus-RC-Tool repository.
- `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`.
- Real project source checkouts.
- AI client configuration files.
- WebUI / MCP schema outside this repo.

## Stop-line

If verification fails, first classify the failure:

- Current-task regression: fix narrowly and rerun the affected gate.
- Pre-existing release blocker: fix only if it is in CodeLattice and blocks the beta hardening pack.
- External/environmental failure: record exact command and error in closure; do not claim pass.

Do not cross into new language semantics, target project execution, external release publication, or stable runtime promotion.

## Verification Plan

Pre-release docs/script verification:

```bash
cargo fmt --check
git diff --check
cargo test --test mcp_server
cargo test
cargo test --all-features
python3 scripts/real-project-corpus-smoke-test.py
scripts/codelattice-mcp.sh --self-test
scripts/mcp-dogfood.sh
scripts/install-mcp.sh --doctor
```

Artifact verification:

```bash
scripts/package-release.sh
scripts/release-smoke.sh --tarball <tarball-path>
scripts/fresh-clone-smoke.sh --skip-tests
scripts/fresh-clone-smoke.sh
```

Real corpus optional gate:

- If `/tmp/codelattice-real-corpus-smoke` cache exists, run offline strict compare.
- If cache does not exist, run `--list` and `--dry-run --max-targets 3`; do not network clone.

Pre-commit graph gate:

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js detect-changes --repo codelattice --scope all
```

Index refresh after passing verification:

```bash
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze /Users/jiangxuanyang/Desktop/codelattice --force --skip-agents-md --name codelattice
```
