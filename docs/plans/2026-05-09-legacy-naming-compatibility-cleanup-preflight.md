# Legacy Naming Compatibility Cleanup Preflight

> **日期：** 2026-05-09
> **状态：** Preflight / 待 production trial 收口后执行
> **目的：** 规划旧名字清理，避免项目长期叙事和 runtime/API 被 `GitNexus` / `GitNexus-RC` 历史兼容名绑定。
> **Stop-line：** 本文只规划命名治理，不立即改 runtime、CLI flag、crate/bin 名、repo 名、schema 字段或 GitNexus-RC adapter。

---

## 一、背景

当前项目已经从“复刻某个现有工具形态”收束为“独立 Rust/Cangjie 本地代码上下文核心”。但历史推进中仍存在大量旧命名：

- `GitNexus Rust Core`
- `GitNexus-RC adapter`
- `--format gitnexus-rc`
- `gitnexus-rust-core-cli`
- `rust-core-bridge-adapter`
- 文档里的 `GitNexus-RC` handoff / governance / compatibility 描述

这些名字短期仍有价值，因为当前需要和过渡消费端、历史治理文档、兼容 bridge 对齐。但它们不应该继续成为长期产品身份。

结论：**旧名字清理应该分层治理，不能一刀切。**

---

## 二、清理原则

1. **先清 public-facing，后清 runtime/API。**
   README、脚本提示、公开文档标题可以先中性化；CLI flag、crate/bin、schema、adapter 模块名需要兼容策略。

2. **保留旧名字作为 compatibility alias。**
   任何已经被测试、脚本或下游消费端使用的旧名字，不能直接删除。新增中性入口后，旧入口先作为 alias 保留。

3. **不在 production trial 收口前做大规模改名。**
   当前优先级是 Rust + Cangjie 本地分析核心可生产试用。命名治理不能打断 smoke、quality gate 和输出合同稳定。

4. **历史事实不强行抹掉。**
   计划文档、closure review、跨仓 handoff 中描述历史来源时，可以保留 `GitNexus-RC`，但要明确这是 legacy / compatibility context。

5. **代码可读性优先于表面洁净。**
   若某个名字代表真实兼容协议，例如 `gitnexus-rc` format，短期应保留并文档化为 legacy compatibility format，而不是无测试迁移。

---

## 三、命名分层

| 层级 | 示例 | 策略 |
|------|------|------|
| Public-facing docs | README 标题、快速开始、脚本提示 | 优先清理为中性说法 |
| Compatibility docs | consumer contract、bridge preflight | 保留旧名，但标注 legacy / compatibility |
| CLI format names | `--format gitnexus-rc` | 新增中性 alias 后保留旧 alias |
| Binary / crate name | `gitnexus-rust-core-cli` | alpha trial 后单独决策，不在本轮改 |
| Source module path | `rust-core-bridge-adapter` | 仅在有 adapter shim 和测试覆盖时改 |
| Historical governance | handoff、closure review、GitNexus-RC links | 保留历史名，不追求全量替换 |
| Repo / product name | 仓库名、最终产品名 | 最后决策，不能混入普通 cleanup |

---

## 四、推荐阶段

### Phase A：Inventory（docs-only）

目标：建立旧名出现位置清单。

建议命令：

```bash
rg -n "GitNexus|GitNexus-RC|gitnexus-rc|gitnexus-rust-core|rust-core-bridge" README.md docs scripts crates -S
```

输出分类：

- public-facing should-clean
- compatibility alias keep
- historical keep
- runtime/API needs migration
- unsafe-to-change

### Phase B：Public-facing cleanup

目标：让公开入口先中性化。

可改：

- README 标题和开头描述
- scripts 输出文案
- docs/plans/README.md 当前状态总结中的产品叙事
- `consumer` / `bridge` 说明中的标题措辞

不可改：

- CLI flag
- binary/crate name
- adapter module path
- test fixture 文件名
- schema 字段

### Phase C：Neutral format alias

目标：新增中性输出格式名，同时保留旧名。

候选名需要另行决策，例如：

- `context-graph`
- `context-graph-v0`
- `bridge-json`
- `compat-json`

验收：

- 新旧 alias 输出等价。
- README 默认示例使用中性名。
- `--format gitnexus-rc` 保留为 legacy alias。
- tests 覆盖 alias equivalence。

### Phase D：Runtime/API rename migration

目标：在不破坏下游的前提下逐步迁移代码内部名称。

前置条件：

- alpha production trial 已通过。
- Tool / 下游 adapter 已经知道旧名是 compatibility alias。
- 所有旧入口都有新入口替代。

可选动作：

- 模块名从 legacy-specific 名字迁移到 neutral 名字。
- 保留 re-export / shim。
- 更新测试名和文档名。
- closure review 记录兼容窗口。

### Phase E：Final project identity

目标：最终产品名、binary 名、repo 名、crate 名统一。

这一步需要用户单独决策，不自动推进。

---

## 五、短期建议

production trial 收口前只做两件事：

1. **继续 public-facing cleanup。**
   清理 README / scripts / 新增 docs 中不必要的旧名暴露。

2. **把旧格式名标成 legacy compatibility alias。**
   `--format gitnexus-rc` 可以继续存在，但公开路线要说明它不是长期品牌名。

不要做：

- 一次性 rename crate/bin。
- 删除 `--format gitnexus-rc`。
- 重命名 adapter 目录。
- 改 GitNexus-RC 已落地 adapter 路径。
- 为了命名清洁破坏现有 tests / scripts。

---

## 六、验收清单

- [ ] 旧名 inventory 已生成并分类。
- [ ] README 默认叙事不再把项目描述为复刻某个旧产品形态。
- [ ] public-facing scripts 输出使用中性称呼。
- [ ] 兼容 bridge 文档明确 `gitnexus-rc` 是 legacy compatibility format。
- [ ] 若新增中性 format alias，新旧 alias 输出一致。
- [ ] 旧 alias 有测试保护，不会被误删。
- [ ] Historical docs 中保留旧名但不作为未来主线表达。
- [ ] runtime/API rename 有独立 execution card，不混入 production trial 收口。

---

## 七、与当前路线的关系

本计划不阻塞 alpha production trial。

当前主线仍是：

1. Rust + Cangjie 本地分析核心可生产试用。
2. 输出合同、quality gates、smoke targets 稳定。
3. AI 可以消费 summary / quality / graph artifact。
4. 命名治理在生产试用收口后逐步推进。

旧名字清理是独立化工程的一部分，但不是当前最紧急的能力缺口。
