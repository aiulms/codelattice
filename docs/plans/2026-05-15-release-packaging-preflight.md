# Release Packaging Preflight — 2026-05-15

## 目的

将 CodeLattice 从"开发仓库可用"推进到"外部 beta 用户可安装、可验证、可升级"的状态。

## 起点

- HEAD: `c44b51d` on `gitcode/master`
- Workspace version: `0.1.0` → bump to `0.13.0-beta.1`
- MCP contract version: `0.13.0`
- 22 MCP tools
- 89 MCP integration tests, 93 project-model tests
- Fixtures: Rust (stable), Cangjie (stable), ArkTS (production trial), TypeScript (Phase A)

## 已有基础设施

- `scripts/package-release.sh` — tarball + manifest + sha256
- `scripts/release-smoke.sh` — tarball unpack + verify
- `scripts/fresh-clone-smoke.sh` — simulated external clone
- `scripts/check-release-metadata.sh` — versioning/changelog validation
- `scripts/install-release.sh` — GitCode release download + install
- `scripts/install-mcp.sh` — build + configure
- `scripts/promote-to-local-tool.sh` — promote to stable runtime
- `scripts/codelattice-mcp.sh` — MCP wrapper with --self-test
- `docs/release-versioning.md` — versioning policy
- `docs/release-install.md` — install guide
- `docs/release-packaging.md` — packaging docs
- `docs/getting-started.md` — getting started
- `CHANGELOG.md` — exists but stale (only v0.1.0 entries)

## 计划

1. Bump version to `0.13.0-beta.1`
2. Update versioning doc for beta status + language support labels
3. Update CHANGELOG with 7 new features
4. Create `scripts/release-manifest.sh` standalone manifest generator
5. Fix package-release.sh manifest (remove local path leak)
6. Add `docs/release/upgrade.md` with upgrade/rollback/cache cleanup
7. Create `docs/release/smoke-matrix.md`
8. Polish README for external beta audience
9. Create closure doc

## 约束

- 不修改 GitNexus-RC / GitNexus-RC-Tool
- 不修改真实项目
- 不做 WebUI
- 不更名 Cargo package/bin
- 不自动 promote
- 不新增 LLM/embedding 依赖
