# 从 GitNexus-RC 迁移

> **日期：** 2026-05-01
> **类型：** 迁移指南
> **状态：** 初稿

---

## 目的

本文档记录从 GitNexus-RC 迁移到 `gitnexus-rust-core` 的内容，以及不迁移的内容。它建立了研究账本和实现契约 workspace 之间的边界。

---

## 迁移什么

### 语言事实

| 内容 | 迁移目标 | 说明 |
|---------|-----------------|-------|
| ProjectModel 数据模型 | Rust structs（PackageModel、TargetModel、WorkspaceModel） | 从 GitNexus-RC 5刀 implementation 冻结 |
| Cargo manifest 扫描规则 | `scanForCargoManifests` → Rust 模块 | 仅 manifest-derived 模型，不执行 `cargo metadata` |
| Workspace/root/target 解析 | 解析算法 | Nearest root wins，target 隔离 |
| No-edge 策略 | 歧义处理 | 优先 no false edge |
| Confidence/reason 策略 | 16 个 reason codes | 结构化为 Rust enum |
| Golden fixture 语料库 | 14 个 ProjectModel fixtures | expected.json truth 在 GitNexus-RC |
| Fixture 断言 | Integration test expected facts | sourceLineage 追踪 GitNexus-RC 来源 |
| 已知局限 | documented limitations | 无自动扩展 |

### 策略文档

| 文档 | 来源 | 状态 |
|----------|--------|--------|
| No-edge 策略 | GitNexus-RC RISK_LEDGER.md | 已迁移为 decision doc |
| Confidence 分层策略 | GitNexus-RC GOVERNANCE.md | 已迁移为 decision doc |
| Stop-line 列表 | GitNexus-RC rust-core preflight | 已冻结 |

### 设计资产

| 资产 | 来源 | 说明 |
|-------|--------|-------|
| ProjectModel 模块设计 | `plans/2026-05-01-rust-core-project-model-module-design.md` | 主要设计输入 |
| Golden fixture 规格 | `plans/2026-05-01-rust-core-project-model-golden-fixture-spec.md` | expected.json schema |
| Output comparison harness | `plans/2026-05-01-rust-core-project-model-output-comparison-harness-design.md` | Phase 4 占位符 |

---

## 不迁移什么

### TypeScript-specific 形状

| 内容 | 原因 | Rust-core 替代方案 |
|---------|--------|---------------------|
| TypeScript `LanguageProvider` 接口 | 绑定 TS 类型、Node fs、LadybugDB | LanguageAdapter trait（新设计） |
| TypeScript phase/worker 形状 | 服务 GitNexus pipeline | 新 pipeline 设计 |
| Vitest helper 函数 | 验证工具 | Rust golden runner |
| npm scripts | 本地研究工具链 | Cargo commands |

### 产品外壳

| 内容 | 原因 | Rust-core 替代方案 |
|---------|--------|---------------------|
| LadybugDB storage APIs | GitNexus 产品层 | JSON graph artifact 输出 |
| MCP server | GitNexus 产品层 | Stop-line（MVP） |
| Web/UI | GitNexus 产品层 | Stop-line（MVP） |
| npm/Vite 工具链 | GitNexus 产品层 | Cargo.toml |

### 验证 Harness 决策

| 内容 | 原因 | 说明 |
|---------|--------|------|
| 本地 vendor Tree-Sitter-Cangjie ABI workaround | 验证 harness decision | 在 Rust 中重新评估 |
| Global fallback 行为 | 有 false-positive 风险 | 显式 low-confidence fallback |
| Node fs API 用法 | Node 特有 | Rust std::fs 或 trait abstraction |
| Test helper 命名约定 | 验证工具 | 新命名 |

### 历史记录

| 内容 | 原因 | 说明 |
|---------|--------|------|
| GitNexus-RC closure review 历史 | 研究账本 | 留在 GitNexus-RC |
| Execution card write sets | 研究记录 | 留在 GitNexus-RC |
| Implementation logs | 研究产物 | 留在 GitNexus-RC |

---

## GitNexus-RC 的角色

GitNexus-RC 仍是 **历史记录来源**：
- 所有 preflight、execution card、closure review 文档
- 所有 fixture 源文件和 expected.json
- 所有 integration test 断言
- 所有 reason/confidence codes 及其使用上下文
- 所有已知局限及其发现证据

**gitnexus-rust-core** 是 **实现契约 workspace**：
- 已冻结的设计规格
- 数据模型草案
- Trait/API 契约
- 策略决策
- 此 workspace **不修改** GitNexus-RC

---

## 同步策略

### Fixture 语料库

Golden fixture 源文件留在 GitNexus-RC：
- `gitnexus/test/fixtures/lang-resolution/rust-cargo-root-baseline/`
- 等等

此 workspace 未来可复制或 vendor fixture snapshots，但不复制 GitNexus-RC 历史日志。

### 设计文档

设计文档在冻结时从 GitNexus-RC 复制：
1. 原始 preflight/closure reviews 留在 GitNexus-RC
2. 关键决策在 `gitnexus-rust-core` decision docs 中摘要
3. Source 链接指回 GitNexus-RC 获取完整上下文

### Schema 变更

如果 GitNexus-RC graph schema 变更：
1. 变更**不自动传播**到 `gitnexus-rust-core`
2. 变更需要单独迁移评估
3. 破坏性变更触发 `gitnexus-rust-core` 新 preflight

---

## 迁移原则

1. **可移植规则优先于验证工具形状** — 迁移语言事实，不迁移 helper 函数
2. **显式优于隐式** — 无 global fallback；low-confidence 必须文档化
3. **Fixture-first** — 设计由 fixtures 验证，不基于抽象架构
4. **遵守 stop-line** — MVP 范围已冻结，无范围蔓延
5. **Source attribution** — 每个迁移决策链接到 GitNexus-RC source

---

## 未来工作

- [ ] 丰富 source-map.md 的字段级映射
- [ ] 记录 Cangjie adapter 迁移计划（在 Rust ProjectModel 之后）
- [ ] 定义 gitnexus-rust-core 的 Cargo workspace 结构

---

*来源：GitNexus-RC docs/language-support/plans/2026-04-28-rust-core-rebuild-preflight.md § 3-4*
