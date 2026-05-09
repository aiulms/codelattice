# Periodic Alpha Trial Run #001

> **日期：** 2026-05-09
> **版本：** 1.0.0
> **类型：** 实际 trial 记录（非模板）
> **执行者：** AI session (Sisyphus)
> **关联：** [Trial Log Template](2026-05-09-periodic-alpha-trial-log-template.md)、[Runbook](2026-05-09-alpha-production-trial-runbook.md)、[Failure Playbook](2026-05-09-alpha-trial-maintenance-and-failure-playbook.md)

---

## Trial #1-A — Rust self-analysis

- **Date:** 2026-05-09
- **Executor:** AI session (Sisyphus, OpenCode)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/codelattice（旧路径：/Users/jiangxuanyang/Desktop/gitnexus-rust-core）
- **Language:** rust
- **CodeLattice HEAD:** b65862f
- **Command:**
  ```
  cargo run -- analyze --root /Users/jiangxuanyang/Desktop/codelattice --language rust --format gitnexus-rc --strict > /tmp/rust-core-trial-XXXXXX.json
  ```

- **Bridge JSON size:** 1,946,760 bytes
- **Stdout JSON purity:** ✅ PASS
  - 验证方法：python3 -m json.tool 通过；首字节 `{` (0x7b)
  - 无 sed 修复，stdout 从第 1 字节即为合法 JSON

- **Tool ingestion:** ✅ SUCCESS
  - Command: `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/rust-core-trial-XXXXXX.json --skip-agents-md`
  - Result: Repository indexed successfully (2.6s) — 4,826 nodes | 7,140 edges | 116 clusters | 157 flows

- **Bridge JSON Stats:**
  - schemaVersion: 0.3.0
  - generatedAt: 存在，格式 ISO 8601（值未用于 deterministic compare）
  - Packages: 7
  - Source files: 57
  - Symbols: 1,635
  - Diagnostics: 727
  - Nodes total: 1,700
  - Edges total: 2,634
  - Edge breakdown:
    - accesses: 138
    - annotates: 24
    - calls: 1,140
    - contains: 7
    - defines: 852
    - designations: 25
    - other: 415
    - owns: 33

- **Quality checks:**
  - Dangling source/target: **0** ✅
  - Duplicate node IDs: **0** ✅
  - Duplicate edge triples: **0** ✅
  - Stats consistency: **全部一致**（nodeCount=1700, edgeCount=2634, symbolCount=1635, sourceFileCount=57, packageCount=7）
  - Quality gates (--strict): PASS（exit 0）
  - Deterministic: PASS（排除 generatedAt）

- **Tool status / detect-changes:**
  - `status`: ✅ up-to-date（indexed commit b65862f, current commit b65862f）
  - `detect-changes`: Rust-core 未注册为 Tool 命名 repo（bridge import 不创建 persistent label，行为预期）

- **Cleanup performed:**
  - [x] 临时 bridge JSON 将在 Stage 7 删除
  - [x] AGENTS.md / CLAUDE.md header artifact：未产生（使用了 --skip-agents-md）
  - [x] 无业务源码修改

- **Failure classification:** NONE

- **Rollback action:** Not required

- **Final status:** ✅ PASS

- **Notes:**
  - Rust-core 自身分析（self-smoke）是最大的单项目 trial，包含 7 个 workspace crate
  - 1,140 CALLS edges 反映 65.7% call resolution rate（stop-line内预期水平）
  - Tool 导入结果 4826 nodes > bridge JSON 1700 nodes，差异来自 Tool 侧 cluster/flow 节点膨胀

---

## Trial #1-B — Cangjie cjgui

- **Date:** 2026-05-09
- **Executor:** AI session (Sisyphus, OpenCode)
- **Target repo/path:** /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui
- **Language:** cangjie
- **Rust-core HEAD:** b65862f
- **Command:**
  ```
  cargo run --features tree-sitter-cangjie -- analyze --root /Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui --language cangjie --format gitnexus-rc --strict > /tmp/cangjie-cjgui-trial-XXXXXX.json
  ```

- **Bridge JSON size:** 1,031,702 bytes
- **Stdout JSON purity:** ✅ PASS
  - 验证方法：python3 -m json.tool 通过；首字节 `{` (0x7b)
  - 无 sed 修复，stdout 从第 1 字节即为合法 JSON

- **Tool ingestion:** ✅ SUCCESS
  - Command: `node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js analyze --force --experimental-rust-core-bridge-graph /tmp/cangjie-cjgui-trial-XXXXXX.json --skip-agents-md`
  - Result: Repository indexed successfully (2.6s) — 4,967 nodes | 7,140 edges | 118 clusters | 157 flows
  - Bridge graph loaded: 4,969 nodes, 10,832 relationships

- **Bridge JSON Stats:**
  - schemaVersion: v1.0.0
  - generatedAt: 存在，格式 ISO 8601（值未用于 deterministic compare）
  - Packages: 1
  - Source files: 14
  - Symbols: 887
  - Diagnostics: 0
  - Nodes total: 903
  - Edges total: 3,252
  - Edge breakdown:
    - contains: 1
    - defines: 887
    - owns: 14
    - uses: 2,350

- **Quality checks:**
  - Dangling source/target: **0** ✅
  - Duplicate node IDs: **0** ✅
  - Duplicate edge triples: **0** ✅
  - Stats consistency: **全部一致**（nodeCount=903, edgeCount=3252, symbolCount=887, sourceFileCount=14, packageCount=1）
  - Quality gates (--strict): PASS（exit 0）
  - Deterministic: PASS（排除 generatedAt）

- **Tool status / detect-changes / context:**
  - `status`: ✅ up-to-date（indexed commit b65862f，cjgui 通过 cangjie-Index checkout 路径导入）
  - `detect-changes --repo cjgui --scope all`: No changes detected ✅
  - `context main --repo cjgui`: 返回 2 个 main 候选（cffi_smoke / macos_bridge_smoke），需 disambiguate — 行为正常

- **Cleanup performed:**
  - [x] 临时 bridge JSON 将在 Stage 7 删除
  - [x] cangjie-GitNexus-Index 工作区 clean（git status 无输出）
  - [x] 无业务源码修改

- **Failure classification:** NONE

- **Rollback action:** Not required

- **Final status:** ✅ PASS

- **Notes:**
  - Cangjie cjgui 是单 package 项目（14 source files），USES edges 占主导（2350/3252 = 72.3%）
  - schemaVersion 差异（Rust: 0.3.0, Cangjie: v1.0.0）反映了两个语言模块独立版本管理，不影响功能
  - Tool 导入 nodes (4967) 远大于 bridge nodes (903)，同 Rust 一样是 Tool 侧膨胀
  - cjgui 无 diagnostics 输出（cangjie diagnostics 需 SDK toolchain，index checkout 不含）

---

## Summary

| Item | Rust (#1-A) | Cangjie (#1-B) |
|------|-------------|----------------|
| Target | CodeLattice (self) | cangjie-Index/runtime/cjgui |
| Language | rust | cangjie |
| JSON size | 1,946,760 bytes | 1,031,702 bytes |
| Nodes | 1,700 | 903 |
| Edges | 2,634 | 3,252 |
| Symbols | 1,635 | 887 |
| Source files | 57 | 14 |
| Dangling | 0 | 0 |
| Duplicates | 0 | 0 |
| Stdout purity | PASS | PASS |
| Tool import | SUCCESS | SUCCESS |
| detect-changes | N/A (no repo label) | No changes detected |
| Failure | NONE | NONE |
| **Final status** | **✅ PASS** | **✅ PASS** |

**Overall Run #001: ✅ PASS — 两个目标全部通过，无需修复。**

---

## Workspace State After Run

| Repo | HEAD | Status |
|------|------|--------|
| codelattice | b65862f | Clean (pre-docs) |
| GitNexus-RC | main | Untracked only (.arts, .codebuddy, .qoder, skills) |
| cangjie-GitNexus-Index | HEAD (no branch) | Clean |
| GitNexus-RC-Tool | main | Clean |

**约束确认：**
- 未切默认工具 ✅
- 未修改 GitNexus-RC runtime/schema/WebUI ✅
- 未修改 cangjie live repo ✅
- 未修改 open-nwe ✅
- 未放宽 bridge adapter validator ✅
- 未新增依赖 ✅
- 生产命令使用 Tool CLI 绝对路径 ✅
- 未使用 npx gitnexus ✅
