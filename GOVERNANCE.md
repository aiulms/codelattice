# GitNexus Rust-core Governance

最后更新：2026-05-09

## 治理来源

GitNexus Rust-core 是 GitNexus 项目的 Rust 实现主体。治理权威来自 GitNexus-RC：

- **语言支持决策**：GitNexus-RC `docs/language-support/GOVERNANCE.md`
- **任务追踪**：GitNexus-RC `docs/language-support/TASK_TRACKER.md`
- **风险记录**：GitNexus-RC `docs/language-support/RISK_LEDGER.md`
- **计划文档**：GitNexus-RC `docs/language-support/plans/`

Rust-core 本地治理仅覆盖 Rust-core 自身的开发流程、code quality 和 fixture 管理。

## 开发流程

### Pipeline

1. **Preflight** — 设计 + 风险评估，记录决策和 write/forbidden set
2. **Execution Card** — 冻结 implementation scope（write set / forbidden set / AC / stop-line）
3. **Implementation** — 编写代码 + fixtures + tests
4. **Closure Review** — 封账 landed reality / residual risks / next opening
5. **Status Consolidation** — 跨多轮 work 的状态收束

### Commit Policy

- `cargo fmt --check` + `cargo test` + `git diff --check` 必须 pass
- Commit 后 push gitcode master
- Push 失败时记录错误，继续后续低风险工作
- 不做 destructive git 操作

### Fixture Policy

- 每个 fixture 必须标注 `compile-valid` 或 `static-analysis-only`
- `compile-valid` fixture 必须通过 `rustc --edition 2021 --crate-type lib` 验证
- `static-analysis-only` fixture 测试 AST 分析能力，不代表 Rust 语义真值
- `expected-*.json` golden fixture 必须 exact match（不含 known skip）

### Comment Policy

- 语义代码注释默认使用中文，必要英文术语保留
- 必须注释：ambiguous / no-edge / low-confidence / fallback / dedupe / stop-line / confidence reason

## 禁止操作

- 不改 GitNexus-RC runtime / adapter / graph schema / package
- 不改 live repo 源码（open-nwe / cangjie / warp / openfang）
- 不跑 production analyze
- 不做 destructive git 操作
- 不清理未知 untracked 文件
- 不触碰 .env / secrets / production credentials

## Workspace Boundary

| 目录 | 角色 | 可写性 |
|------|------|--------|
| `gitnexus-rust-core` | Rust implementation | 可写 |
| GitNexus-RC | Governance center + TS dev | 可写（docs-only for Rust work） |
| GitNexus-RC-Tool | Stable CLI | 只读 |
| Live repos | open-nwe / cangjie / warp / openfang | 默认禁止 |

## Closure Recording Policy

- **日常 implementation closure** → Rust-core 本地（`docs/plans/` 或 GitNexus-RC `docs/language-support/plans/`）
- **Cross-repo handoff / milestone** → GitNexus-RC `docs/language-support/plans/`
- **Risk updates** → GitNexus-RC `docs/language-support/RISK_LEDGER.md`
- **Tracker updates** → GitNexus-RC `docs/language-support/TASK_TRACKER.md`
