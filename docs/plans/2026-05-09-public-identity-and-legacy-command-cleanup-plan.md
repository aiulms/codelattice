# Public Identity and Legacy Command Cleanup Plan

> **日期：** 2026-05-09
> **版本：** 1.1.0
> **状态：** Phase 1 已完成（2026-05-09），残留清单已建立
> **关联：** [Alpha Production Trial Runbook](2026-05-09-alpha-production-trial-runbook.md)

---

## 一、背景

Rust-core 从 GitNexus-RC 独立出来后，存在一些旧名和措辞需要清理。但这些清理不能影响已稳定的 bridge compatibility flag 和 alpha trial 操作。

**核心原则：**
- Rust-core 是独立 Rust-native 分析核心
- GitNexus-RC 是过渡生产环境和 adapter consumer
- Tool checkout 是当前稳定 CLI 分发路径
- 先清 public-facing，再清 internal，最后评估 CLI flag

---

## 二、必须立即清理

### 2.1 公开 README / scripts / runbook 中误导性旧名

| 位置 | 当前措辞 | 建议改为 |
|------|----------|----------|
| 公开文档中 "GitNexus-RC adapter" | 可能暗示从属关系 | "下游消费方 adapter" 或 "bridge consumer" |
| 脚本注释中的旧项目引用 | 历史遗留 | 中性表述 |

### 2.2 生产命令中的 npx gitnexus

| 位置 | 问题 | 修复 |
|------|------|------|
| AGENTS.md / CLAUDE.md 中自动生成的 block | Tool 写入 `npx gitnexus analyze` | 使用 Tool CLI 绝对路径；这些 block 不应提交 |
| 任何文档中的 `npx gitnexus` 指令 | 与实际 Tool 路径不一致 | 替换为 `node <Tool绝对路径>/cli/index.js ...` |

### 2.3 把 GitNexus-RC 说成 Rust-core 默认消费方的措辞

Rust-core 是独立核心，GitNexus-RC 是其中一个消费方（虽然是当前唯一一个）。文档中应避免暗示从属或默认关系。

---

## 三、暂时保留

| 项目 | 原因 |
|------|------|
| `--format gitnexus-rc` CLI flag | Bridge compatibility format 名称，改 flag 破坏下游消费方 |
| GitNexus-RC adapter 相关历史文档 | 历史事实记录，不应修改 |
| closure review 中的历史事实 | 历史文档，不应回溯修改 |
| `crates/cli/src/rust_bridge.rs` 文件名 | 内部实现，不影响用户 |
| GitNexus-RC AGENTS.md / GUARDRAILS.md 中的引用 | 不属于 Rust-core 管辖范围 |

---

## 四、命名原则

1. **Rust-core** = 独立 Rust-native 本地代码上下文分析核心
2. **GitNexus-RC** = 过渡生产环境，包含 TypeScript adapter + WebUI + MCP
3. **Tool checkout** = 当前稳定 CLI 分发路径（`GitNexus-RC-Tool/gitnexus/dist/cli/index.js`）
4. **Bridge format** = `--format gitnexus-rc` 产出的 JSON 格式，面向任何兼容消费方
5. **Bridge consumer** = 任何消费 bridge JSON 的下游系统（GitNexus-RC adapter 是其中之一）

---

## 五、未来执行建议

### Phase 1: Public-facing docs/scripts 清理

- 检查 README.md 中是否有 "GitNexus-RC" 直接引用应改为中性表述
- 检查 build.sh / smoke.sh / verify-bridge.sh 注释中的措辞
- 更新 alpha trial runbook 中的措辞一致性

### Phase 2: Internal docs 清理

- architecture/ 下文档的措辞统一
- 源码注释中的旧名（不影响功能，但影响可读性）

### Phase 3: CLI flag / schema 评估

- 评估 `--format gitnexus-rc` 是否需要 rename（不建议短期改，会破坏兼容性）
- 如果未来有第二个消费方，可能需要更中性的 format 名称
- 任何 CLI flag 变更必须以 compatibility alias 方式迁移

---

## 六、明确不要现在做的事

- ❌ 不做大规模 find-and-replace rename
- ❌ 不破坏 `--format gitnexus-rc` 兼容 flag
- ❌ 不改 GitNexus-RC runtime/schema
- ❌ 不在 Rust-core 内部做纯 cosmetic rename（浪费 token 和 review 时间）
- ❌ 不修改历史 closure review 中的事实描述

---

## 七、验收标准

当以下条件满足时，本清理计划可关闭：

1. 所有 public-facing 文档使用中性表述
2. 无 `npx gitnexus` 出现在生产指令中
3. Rust-core README 清晰说明独立身份和适用范围
4. 无误导性从属关系暗示
5. `--format gitnexus-rc` flag 仍然正常工作

---

## 八、Phase 1 执行记录（2026-05-09）

### 审查范围

| 位置 | 文件数 | "GitNexus-RC" 引用 | npx gitnexus | 需修改 |
|------|--------|-------------------|--------------|--------|
| README.md | 1 | 0 | 0 | 无（已中性） |
| scripts/*.sh | 4 | 0 | 0 | 无（已中性） |
| docs/architecture/*.md | 9 | ~109 | 0 | 无（均为桥接适配器接口事实描述） |
| docs/plans/ (当前文档) | 5 | ~15 | 0 | 小幅修正（runbook 适用范围措辞） |
| docs/decisions/*.md | 1 | 0 | 0（仅"禁止使用"语境） | 无 |

### 已修正

- runbook §header 适用范围：`GitNexus-RC Tool bridge ingestion` → `Tool bridge ingestion（experimental flag）`
- runbook §二 适用范围表：同上

### 保留项（残留清单，Phase 2 处理）

| 类别 | 数量 | 说明 |
|------|------|------|
| docs/architecture/ 桥接适配器接口描述 | ~109 | 均为事实性引用（字段映射、类型对照、消费流程），描述的是与 GitNexus-RC adapter 的具体对接协议，改成中性反而会降低精确度 |
| docs/plans/ 历史 closure review | 大量 | 历史事实记录，明确不改 |
| `--format gitnexus-rc` / `--experimental-rust-core-bridge-graph` | N/A | CLI flag，改则破坏兼容性 |
| AGENTS.md 治理来源引用 | 1 | 事实性（GitNexus-RC 是治理来源仓库） |

### 结论

Phase 1 发现 Rust-core 的 public-facing docs/scripts 命名已基本中性化（上一轮 public identity cleanup 已完成主要工作）。残留的 `GitNexus-RC` 引用集中在桥接适配器接口文档和历史 closure review 中，均属事实性描述。Phase 2（internal docs）和 Phase 3（CLI flag）暂不执行。
