# Release Packaging Closure — 2026-05-15

## 版本

- **产品版本**: `0.13.0-beta.1`
- **MCP 合约版本**: `0.13.0`
- **源码提交**: `c44b51d` (prerelease HEAD, pending this commit)
- **MCP 工具数量**: 22

## Tarball

```text
dist/codelattice-0.13.0-beta.1-darwin-arm64.tar.gz
SHA256: c31229fc84a88ecfe4951d5c88236f6fec0daefe48bb5dcf7602363063bca408
```

### Manifest 摘要

| 字段 | 值 |
|------|-----|
| releaseVersion | 0.13.0-beta.1 |
| platform | darwin-arm64 |
| serverVersion | 0.13.0 |
| toolCount | 22 |
| binarySha256 | 1c9bf66855c6691bfed0f9744583dd736eb9056d89d5834259cf41ff440e6402 |
| releaseStatus | external-beta |

## Smoke 结果

### 测试

| Suite | 结果 |
|-------|------|
| MCP integration tests | 89/89 PASS |
| Project model tests | 93/93 PASS |
| All-features tests | 199/199 PASS |
| Cache smoke | 6/6 PASS |
| Dogfood | 22/22 PASS |

### 脚本

| Script | 结果 |
|--------|------|
| cargo fmt --check | ✅ Clean |
| git diff --check | ✅ Clean |
| codelattice-mcp.sh --self-test | ✅ Pass |
| install-mcp.sh --doctor | ✅ 6/7 (cangjie feature not in promoted runtime, expected) |
| package-release.sh | ✅ Tarball generated |
| release-smoke.sh | ✅ Checksum + self-test + tools + Rust/Cangjie fixtures |
| fresh-clone-smoke.sh --skip-tests | ✅ Build + promote + MCP + fixtures |
| release-manifest.sh | ✅ Valid JSON |
| detect-changes | Low risk, 8 files (docs only) |

## 已创建/更新文件

### 新增

- `scripts/release-manifest.sh` — 独立 manifest 生成器
- `docs/release/upgrade.md` — 升级/回滚/cache 清理指南
- `docs/release/smoke-matrix.md` — smoke 矩阵文档
- `docs/plans/2026-05-15-release-packaging-preflight.md` — preflight 文档
- `docs/plans/2026-05-15-release-packaging-closure.md` — 本文档

### 更新

- `Cargo.toml` — version bumped to `0.13.0-beta.1`
- `CHANGELOG.md` — 新增 0.13.0-beta.1 section 含 7 个新特性 + known limitations
- `docs/release-versioning.md` — 新增 beta 状态、语言支持标签、MCP 合同版本说明
- `scripts/package-release.sh` — manifest 移除 sourceRepo 本地路径
- `scripts/codelattice-mcp.sh` — cache_status 检查适配嵌套格式
- `scripts/install-mcp.sh` — doctor cache_status 检查适配嵌套格式
- `README.md` — External beta polish (由 writing agent 完成)

## Known Limitations

- TypeScript: 无 path alias 解析、无 monorepo/workspace 支持、无 TSX framework hints
- ArkTS: struct 关键字被解析为 ERROR（workaround 已有）、无 @Builder/@Extend、无完整 ArkUI 语法树
- Persistent cache: 无 per-symbol incremental recompute
- 调用边为启发式，带 confidence/reason，非编译器验证
- 不执行项目脚本
- 不是编译器、IDE、语言服务器或托管服务

## Not GA 声明

此版本为 External Beta。本地 production trial 已通过（Rust / Cangjie），但不是 GA release。Beta 期间 minor 版本可能包含 breaking MCP 输出变化。

## 未触碰项

- ✅ 未修改 GitNexus-RC runtime/schema/WebUI
- ✅ 未修改 GitNexus-RC-Tool
- ✅ 未修改真实项目（CoolMallArkTS / harmony-utils / HarmonyOS-Examples）
- ✅ 未修改 Codex/opencode/Claude 实际配置
- ✅ 未自动 promote 到 `/Users/jiangxuanyang/Desktop/CodeLattice-Tool`
- ✅ 未做 WebUI
- ✅ 未更名 Cargo package/bin
- ✅ 未新增 LLM/embedding 依赖

## 下一步建议

1. 创建 git tag `v0.13.0-beta.1`（需用户明确要求）
2. 在 GitCode 创建 beta release page
3. TypeScript path alias / monorepo 支持
4. TSX framework hints（React component detection）
5. 更深层的 per-symbol incremental recompute
6. 多平台 release artifact（Linux、Windows）
