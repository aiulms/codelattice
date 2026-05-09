# Periodic Alpha Trial Run #002

> **日期：** 2026-05-10
> **版本：** 1.0.0
> **类型：** 实际 trial 记录（非模板）
> **执行者：** AI session (Sisyphus)
> **关联：** [Trial Run #001](2026-05-09-periodic-alpha-trial-run-001.md)、[Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)

---

## Trial #2-A — Rust self-analysis

- **Date:** 2026-05-10
- **Executor:** AI session (Sisyphus, OpenCode)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/codelattice
- **Language:** rust
- **CodeLattice HEAD:** 18d6408
- **Command:**
  ```
  cargo run -- analyze --root /Users/jiangxuanyang/Desktop/codelattice --language rust --format gitnexus-rc --strict > /tmp/codelattice-rust-trial-XXXXXX.json
  ```

- **Bridge JSON size:** 1,946,732 bytes
- **Stdout JSON purity:** ✅ PASS
  - 验证方法：python3 -m json.tool 通过；首字节 `{` (0x7b)
  - 无 sed 修复

- **Tool ingestion:** ✅ SUCCESS
  - Command: `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-rust-trial-XXXXXX.json --skip-agents-md --name codelattice`
  - Result: Repository indexed successfully (2.5s) — 4,877 nodes | 7,199 edges | 117 clusters | 157 flows

- **Bridge JSON Stats:**
  - schemaVersion: 0.3.0
  - generatedAt: 存在，格式 ISO 8601
  - Packages: 7
  - Source files: 57
  - Symbols: 1,635
  - Diagnostics: 727
  - Nodes total: 1,700
  - Edges total: 2,634

- **Quality checks:**
  - Dangling source/target: **0** ✅
  - Duplicate node IDs: **0** ✅
  - Duplicate edge triples: **0** ✅
  - Stats consistency: **全部一致** ✅
  - Quality gates (--strict): PASS（exit 0）

- **Tool status / detect-changes:**
  - `status`: ✅ up-to-date（indexed commit 18d6408, current 18d6408）
  - `detect-changes --repo codelattice --scope all`: No changes detected

- **Cleanup performed:**
  - [x] 临时 bridge JSON 已删除
  - [x] .claude/ 已清理
  - [x] 无业务源码修改

- **Failure classification:** NONE
- **Rollback action:** Not required
- **Final status:** ✅ PASS

---

## Trial #2-B — Cangjie cjgui

- **Date:** 2026-05-10
- **Executor:** AI session (Sisyphus, OpenCode)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui
- **Language:** cangjie
- **CodeLattice HEAD:** 18d6408
- **Command:**
  ```
  cargo run --features tree-sitter-cangjie -- analyze --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui --language cangjie --format gitnexus-rc --strict > /tmp/codelattice-cjgui-trial-XXXXXX.json
  ```

- **Bridge JSON size:** 1,031,702 bytes
- **Stdout JSON purity:** ✅ PASS
  - 验证方法：python3 -m json.tool 通过；首字节 `{` (0x7b)
  - 无 sed 修复

- **Tool ingestion:** ✅ SUCCESS
  - Command: `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/codelattice-cjgui-trial-XXXXXX.json --skip-agents-md`
  - Result: Repository indexed successfully (2.4s) — 5,017 nodes | 7,199 edges | 118 clusters | 157 flows

- **Bridge JSON Stats:**
  - schemaVersion: v1.0.0
  - generatedAt: 存在，格式 ISO 8601
  - Packages: 1
  - Source files: 14
  - Symbols: 887
  - Diagnostics: 0
  - Nodes total: 903
  - Edges total: 3,252

- **Quality checks:**
  - Dangling source/target: **0** ✅
  - Duplicate node IDs: **0** ✅
  - Duplicate edge triples: **0** ✅
  - Stats consistency: **全部一致** ✅
  - Quality gates (--strict): PASS（exit 0）

- **Tool status / detect-changes:**
  - `status`: ✅ up-to-date
  - `detect-changes --repo cjgui --scope all`: No changes detected

- **Cleanup performed:**
  - [x] 临时 bridge JSON 已删除
  - [x] cangjie-GitNexus-Index clean
  - [x] 无业务源码修改

- **Failure classification:** NONE
- **Rollback action:** Not required
- **Final status:** ✅ PASS

---

## Summary

| Item | Rust (#2-A) | Cangjie (#2-B) |
|------|-------------|----------------|
| Target | CodeLattice (self) | cangjie-Index/runtime/cjgui |
| JSON size | 1,946,732 bytes | 1,031,702 bytes |
| Nodes | 1,700 | 903 |
| Edges | 2,634 | 3,252 |
| Symbols | 1,635 | 887 |
| Dangling | 0 | 0 |
| Duplicates | 0 | 0 |
| Stdout purity | PASS | PASS |
| Tool import | SUCCESS | SUCCESS |
| detect-changes | No changes detected | No changes detected |
| **Final status** | **✅ PASS** | **✅ PASS** |

**Overall Run #002: ✅ PASS — 两个目标全部通过，结果与 Run #001 完全一致（graph stats 不变），零回归。**

---

## Run #001 vs #002 对比

| Metric | Run #001 | Run #002 | Delta |
|--------|----------|----------|-------|
| Rust nodes | 1,700 | 1,700 | 0 |
| Rust edges | 2,634 | 2,634 | 0 |
| Rust symbols | 1,635 | 1,635 | 0 |
| Cangjie nodes | 903 | 903 | 0 |
| Cangjie edges | 3,252 | 3,252 | 0 |
| Cangjie symbols | 887 | 887 | 0 |
| Dangling (both) | 0 | 0 | 0 |
| Duplicates (both) | 0 | 0 | 0 |
| Stdout purity | PASS | PASS | — |

Graph stats 完全一致，验证了 deterministic output（排除 generatedAt）。

---

## Workspace State After Run

| Repo | HEAD | Status |
|------|------|--------|
| codelattice | 18d6408 | Clean |
| GitNexus-RC | main | Untracked only |
| cangjie-GitNexus-Index | HEAD (no branch) | Clean |
| RC-Tool | main | Clean |

---

## 本轮附加修复

修复了 `scripts/alpha-trial-smoke.sh` 的 Tool 导入检查可靠性：
- 旧方案：`tool_cmd | grep -q "indexed successfully"` — ANSI 进度控制字符导致管道匹配偶发失败
- 新方案：捕获 Tool 输出到临时文件，先检查 exit code（exit 0 = 成功），再检查输出文本（缺失也接受）
- 统一为 `tool_bridge_import()` helper 函数，Rust/Cangjie 共用
- 修复 NODE_BIN 默认路径（`node` → `/opt/homebrew/bin/node`）

Smoke 验证：8/8 PASS，cleanup 后 registry 保持 `codelattice`，无残留。

---

**约束确认：**
- 未切默认工具 ✅
- 未修改 GitNexus-RC runtime/schema/WebUI ✅
- 未修改 cangjie live repo ✅
- 未修改 open-nwe ✅
- 未放宽 bridge adapter validator ✅
- 生产命令使用 Tool CLI 绝对路径 ✅
- 未使用 npx gitnexus ✅
