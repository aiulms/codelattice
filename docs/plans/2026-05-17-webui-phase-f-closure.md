# WebUI Phase F — Closure Review

> **日期:** 2026-05-17 | **状态:** ✅ 完成

## 1. Phase F: Beta Readiness

本阶段是 hardening / polish / docs，不新增功能。

## 2. Runner Contract Tests

| Happy Path (10) | Error Path (11) |
|-----------------|----------------|
| GET health | invalid JSON body |
| GET profiles | missing root |
| POST profile | root not found |
| PUT profile | root is file |
| POST gen-for-profile | unsupported language |
| GET snapshots | path traversal (../bad) |
| GET snapshot | missing snapshot |
| rebuild-index | delete missing |
| DELETE snapshot | missing profile |
| DELETE profile | delete missing profile |

All 21 tests verify `{success,data,error,hint}` response format.

## 3. Browser Smoke

12 checks: HTML serves, JS assets 200, health API, Profiles/Library/Guided/Report/Caution/Generate text verification.

## 4. Beta Sanity

- .codelattice-webui gitignored
- runner binds 127.0.0.1 (no 0.0.0.0)
- uses subprocess.run (not shell=True)
- 5 fixture snapshots: no path leaks
- no npm package files
- all 6 smoke scripts run

## 5. Beta Docs

| Doc | Content |
|-----|---------|
| beta-user-guide.md | Quick start, 10 usage steps |
| beta-safety-boundaries.md | Static-only, no proof, runner safety |
| troubleshooting.md | Common issues + solutions |

## 6. Verification

| 验证项 | 结果 |
|--------|------|
| viewer-smoke | ✅ 56/56 PASS |
| runner-smoke | ✅ 8/8 PASS |
| contract test | ✅ 21/21 PASS |
| browser smoke | ✅ 12/12 PASS + 1 skipped |
| trial | ✅ 15/15 PASS |
| cargo test | ✅ 114 passed |
| MCP self-test + dogfood | ✅ PASS |
| detect-changes | ✅ No changes |

## 7. 下一步

1. True Live MCP Mode
2. Desktop Shell (Tauri/Electron)
3. Graph Deep Interaction
4. Release tagging + distribution
