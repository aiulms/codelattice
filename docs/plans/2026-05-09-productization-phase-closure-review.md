# Productization Phase Closure Review

> **日期：** 2026-05-09
> **版本：** v1.0.0
> **状态：** Closed — 5/5 priorities 完成
> **关联 commits：** `d016b5d`（Unified CLI），`5363eb8`（Bridge Adapter），`9ddf3c7`（Bridge CLI tests + closure），`0b8ed5f`（symbol 分类修复），`9528000`（package_id 解析修复）

---

## 一、Phase 目标回顾

将 gitnexus-rust-core 从"两个语言分析 crate + tests"推进到"可本地试用的复刻版工具骨架"。

五个优先级：
1. Unified CLI Surface（analyze / quality / summary 命令）
2. Unified Output Contract（Rust + Cangjie 共用 wrapper）
3. Quality Command（质量门 JSON 输出 + exit codes）
4. Smoke Targets Config（read-only smoke target 列表）
5. Bridge Preparation（GitNexus-RC 兼容格式 adapter）

---

## 二、Landed Reality

### 2.1 Unified CLI Surface

新增三个产品化 CLI 命令，不破坏现有 `project-model inspect` / `cangjie inspect`：

| 命令 | 用途 | 语言支持 | 输出 |
|------|------|---------|------|
| `analyze` | 完整分析（graph + quality gates） | rust / cangjie / auto | JSON stdout |
| `quality` | 质量门检查 | rust / cangjie（需显式） | JSON + exit code |
| `summary` | 统计摘要（不含完整 graph） | rust / cangjie | JSON stdout |

语言自动检测（`--language auto`）：
- Cargo.toml → rust
- cjpm.toml → cangjie
- 两者都有 → 报错

### 2.2 Unified Output Contract

文档：`docs/architecture/unified-output-contract.md`

核心类型（`crates/cli/src/unified_types.rs`）：
- `GraphSummary`：7 个统计字段（node/edge/symbol/sourceFile/package/diagnostic/callEdge count）
- `QualityGateResult`：gateName + passed + detail
- `LanguageAnalysisResult`：language + root + analyzedAt + schemaVersion + summary + qualityGates + graph

Rust 和 Cangjie 分别实现，被统一顶层 wrapper 包住。

### 2.3 Quality Command

- `quality --language rust`：5 门（duplicate_nodes / duplicate_edges / dangling_source / dangling_target / deterministic）
- `quality --language cangjie`：6 门（以上 5 个 + synthetic_nodes）
- Exit codes：0=pass, 1=fail, 2=ambiguous

### 2.4 Smoke Targets Config

文档：`docs/smoke-targets-config.md`

16 个 smoke targets：
- Tier 1：12 个 repo 内 fixtures（始终可用）
- Tier 2：5 个 machine-local repos（缺失时 graceful skip）

包含 CLI 命令和 Quick Verification Script。

### 2.5 Bridge Preparation

**Bridge 格式 adapter**（`crates/cli/src/bridge_format.rs`，~799 行）：
- 将 Rust/Cangjie graph 转换为 GitNexus-RC 兼容格式
- 归一化 edge 端点字段（source/target → sourceId/targetId）
- 节点按 kind 显式分类（repository / package / sourceFile / symbol）
- 边按类型分组（calls / defines / uses / accesses / designations / imports / contains / owns / annotates / other）
- `--format gitnexus-rc` CLI flag 已接入 analyze 命令

**Bridge Preflight**（`docs/architecture/bridge-preflight.md`）：
- Rust-core vs GitNexus-RC 格式差异矩阵
- 可直接映射字段列表
- 需要 adapter 的差异
- Stop-line 声明

### 2.6 测试覆盖

| 测试类型 | 数量 | 门控 |
|---------|------|------|
| bridge_format unit tests | 7 | always |
| productization integration tests | 11 (no-feature) + 4 (feature) = 15 total | feature-gated |
| language_detect unit tests | 4 | always |
| graph_contract (Rust) | 58 | always |
| graph_contract (Cangjie) | 24 | feature-gated |

---

## 三、Stop-line 验证

| Stop-line | 状态 |
|-----------|------|
| 不修改 GitNexus-RC | ✅ 未触碰 |
| 不修改 GitNexus-RC-Tool | ✅ 未触碰 |
| 不修改 live repo | ✅ 未触碰 |
| 不新增依赖 | ✅ 零新增（serde 已在 workspace） |
| 不做 destructive git | ✅ 未执行 |
| 不做 WebUI/MCP/HTTP | ✅ 未实现 |
| 不做 type inference / trait solving | ✅ 未实现 |
| 不做 production replacement | ✅ 仅为 Rust-core 内部工具 |

---

## 四、Residual Gaps（已知差距）

| Gap | 级别 | 说明 |
|-----|------|------|
| Bridge format symbol 分类过宽 | ~~LOW~~ → **已修复（`0b8ed5f`）** | diagnostic/workspace/module node 不再误计入 symbols；仅匹配 label="symbol" |
| Bridge format 无 package_id 实体化 | ~~LOW~~ → **已修复（`9528000`）** | source-file→target→package 两跳 edge traversal 已实现；31/54 source files 获得 package_id |
| `--language auto` 不做深度检测 | LOW | 仅检查 manifest 文件存在性，不做 language heuristics |
| 非 JSON 格式不支持 | BY DESIGN | 第一版仅做 JSON stdout |
| Bridge format CLI integration tests 仅覆盖 analyze | LOW | quality/summary 命令未支持 --format gitnexus-rc |
| 无本地构建脚本 | LOW（新增 gap） | 非 Rust 开发者试用门槛高 → **本轮（Slice: local-trial-packaging）处理** |

---

## 五、文件清单

### 新增文件

| 文件 | 用途 |
|------|------|
| `crates/cli/src/bridge_format.rs` | GitNexus-RC 兼容格式 adapter |
| `crates/cli/src/unified_types.rs` | 统一输出类型定义 |
| `crates/cli/src/language_detect.rs` | 语言自动检测 |
| `crates/cli/tests/productization_commands.rs` | 产品化 CLI 集成测试（15 tests） |
| `docs/architecture/unified-output-contract.md` | 统一输出契约文档 |
| `docs/architecture/bridge-preflight.md` | Bridge 桥接准备分析 |
| `docs/smoke-targets-config.md` | Smoke 目标配置 |
| `docs/plans/2026-05-09-productization-phase-closure-review.md` | 本文件 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/cli/src/main.rs` | 新增 analyze/quality/summary 命令 + bridge format 调度 + 辅助函数 |
| `crates/cli/Cargo.toml` | 新增 `serde` dependency |
| `README.md` | 新增 Unified Productization CLI 章节 |
| `docs/plans/README.md` | 新增 Productization 线状态总结 |

---

## 六、下一步建议

### 短期（可在 Rust-core 内闭环）
1. ~~**Bridge format 分类精准化**~~ ✅ 已修复（`0b8ed5f`）：partition_rust_nodes 仅匹配 label="symbol"
2. ~~**Bridge format package_id**~~ ✅ 已修复（`9528000`）：edge traversal 两跳查找
3. **Local trial packaging** ✅ 完成（`scripts/build.sh` + `scripts/smoke.sh`）：一键构建 + 快速验证
4. **Bridge format 扩展**：quality/summary 命令也支持 `--format gitnexus-rc`

### 中期（需跨仓协商）
4. **前端消费准备**：与 GitNexus-RC 维护者协商 schema 版本迁移路径
5. **Bridge integration tests**：用 `--format gitnexus-rc` 对 GitNexus-RC 的测试 fixture 做 roundtrip 验证

### 长期（需 stop-line 调整）
6. **MCP/HTTP 消费层**：如果前端消费验证成功，可考虑新增 HTTP API 或 MCP server
7. **单二进制发布**：release build + CI/CD pipeline

---

## 变更记录

| 日期 | 版本 | 变更 |
|------|------|------|
| 2026-05-09 | 1.0.0 | 初始 closure review：5/5 priorities 完成，landed reality，stop-line 验证，residual gaps，下一步建议 |
