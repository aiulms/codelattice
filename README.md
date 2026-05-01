# GitNexus Rust-core 复刻 staging workspace

> **创建时间：** 2026-05-01
> **类型：** Rust-core 复刻 staging workspace + 最小 Rust 工程骨架
> **状态：** Cargo workspace bootstrap 已落地，ProjectModel 仍为 stub

---

## 目的

本 workspace（`gitnexus-rust-core`）是 **GitNexus Rust-core 复刻项目的实现契约 / staging workspace**，现在也包含最小可运行的 Rust 工程骨架。它不是 GitNexus-RC 的子目录。

**与 GitNexus-RC 的关系：**
- GitNexus-RC（`/Users/jiangxuanyang/Desktop/GitNexus-RC`）是 **研究来源与历史账本**。
- `gitnexus-rust-core` 是 **实现契约 workspace**。
- 所有语言支持事实、fixture 语料库和架构决策都源自 GitNexus-RC。

---

## 当前范围

**在范围内：**
- Rust-core ProjectModel 模块设计与数据模型
- Graph schema v0 规格
- LanguageAdapter trait 设计
- Confidence/reason 策略
- Golden fixture 语料库（14 个 ProjectModel fixtures）
- 从 GitNexus-RC 的迁移映射

**在范围外（MVP stop-line）：**
- UI / Web
- MCP server
- rust-analyzer 集成
- 宏展开
- 完整 cfg 求值器
- `cargo metadata` 执行
- 商业分发

---

## 为什么用 Rust

Rust-core 推荐原因：
1. CPU/IO 混合型任务（文件扫描、解析、索引）受益于 Rust 的并发模型和所有权系统
2. Tree-sitter 语法、Cargo 工具链、嵌入式存储有成熟的 Rust 绑定
3. 单二进制分发、可嵌入 library crate
4. 内存安全性和显式错误类型适合表达 parser failure / partial graph / low-confidence fallback
5. Rust enum/trait 适合 LanguageAdapter contract、graph node kind、edge reason、confidence tier

---

## 当前资产

| 资产 | 来源 | 状态 |
|------|------|------|
| 14 个 ProjectModel fixtures | GitNexus-RC | Golden truth |
| expected.json schema | GitNexus-RC | 已冻结 |
| Confidence/reason 策略 | GitNexus-RC | 已冻结 |
| No-edge 策略 | GitNexus-RC | 已冻结 |
| ProjectModel 模块设计 | GitNexus-RC | 已冻结 |
| Cargo workspace 骨架 | 本 workspace | 已落地 |
| CLI stub | 本 workspace | 可输出 contract-compliant JSON |

---

## 命令权威

此 workspace **默认未被 GitNexus 索引**。

查 GitNexus 时，使用 MCP tools 或 Tool CLI 绝对路径：

```bash
# MCP tools（首选）
gitnexus_detect_changes(), gitnexus_impact(), gitnexus_context()

# Tool CLI（MCP 不可用时）
node /Users/jiangxuanyang/Desktop/GitNexus-RC-Tool/gitnexus/dist/cli/index.js <command>
```

**禁止使用 `npx gitnexus`** 做生产分析。`npx` 解析到旧版 npm 发布的 `gitnexus@1.6.1`，缺少 detect-changes、Cangjie 支持和多 repo 功能。

---

## 目录结构

```
gitnexus-rust-core/
  Cargo.toml                         # Cargo workspace
  README.md                          # 本文件
  crates/
    project-model/                   # ProjectModel 输出类型和 stub 生成器
    cli/                             # gitnexus-rust-core CLI
  docs/
    architecture/
      project-model.md               # ProjectModel 职责摘要
      graph-schema-v0.md            # （skeleton）Node/Edge schema 草案
      language-adapter-contract.md  # （skeleton）LanguageAdapter trait 草案
      confidence-reason-policy.md    # （skeleton）Confidence 分层草案
      output-contract.md             # （skeleton）JSON/NDJSON 输出规格
    fixtures/
      fixture-index.md              # 14 个 fixtures 索引
      expected-json-schema.md       # （skeleton）expected.json schema
    decisions/
      no-edge-policy.md             # No-edge 优先于 false edge
      known-limitations.md           # （skeleton）当前已知局限
      command-authority.md          # 此 workspace 命令权威
    migration/
      from-gitnexus-rc.md          # 从 GitNexus-RC 迁移什么
      source-map.md                 # （skeleton）Source truth 映射
  fixtures/
    rust-project-model/
      README.md                     # Fixture 语料库状态
```

---

## 第一目标：ProjectModel

第一个实现目标是 **Rust-core ProjectModel 模块**。

当前已实现：

- `project-model inspect --root <path> --format json`
- 输出 CLI/output contract 要求的 14 个顶层字段
- `diagnostics` 显式包含 `project-model-scan-not-implemented`
- 暂不执行 Cargo manifest scan，避免把 stub 误读为真实 project facts

运行示例：

```bash
cargo run -p gitnexus-rust-core-cli -- project-model inspect --root . --format json
```

来源：
- [Rust-core rebuild preflight](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-04-28-rust-core-rebuild-preflight.md)
- [ProjectModel consolidation handoff](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-project-model-consolidation-handoff-review.md)
- [ProjectModel 模块设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-module-design.md)
- [Golden fixture 规格](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-golden-fixture-spec.md)
- [Output comparison harness 设计](https://github.com/JXY001312/GitNexus-RC/blob/main/docs/language-support/plans/2026-05-01-rust-core-project-model-output-comparison-harness-design.md)

Golden truth：GitNexus-RC 中 14 个 `expected.json` 文件，位于 `gitnexus/test/fixtures/lang-resolution/rust-*/expected.json`。

---

## 下一步

1. **Rust-core ProjectModel manifest scan implementation** — 读取 Cargo.toml，先对齐 baseline/subdirectory fixtures。
2. **Rust-core workspace source-map enrichment** — 细化迁移 source-map 的字段级映射。

---

## 许可证

本项目遵循 GitNexus PolyForm Noncommercial 许可证。参见 [GitNexus-RC LICENSE](https://github.com/JXY001312/GitNexus-RC/blob/main/LICENSE)。
