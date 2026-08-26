# 模块级架构视图 · Preflight

日期：2026-08-24 · 状态：已确认并实施 · 来源：2026-08-24 产品讨论（两客户一引擎、桌面优先）

## 1. 背景与目标

CodeLattice 桌面端（`apps/desktop`）现状：

- `graph-pane` 只渲染 snapshot 给定的符号级/文件级节点平铺图，无层级聚合。
- `dashboard` 有"三层结构骨架"（Package/Module · File · Symbol 计数），但只是计数摘要，不可交互。
- graph schema 已注册 `Module` 节点与 `RESOLVES_TO` 边（§6.7），但 **8 种边全为结构包含关系，无模块间依赖边**。

用户愿景（给人用的一半）：打开项目先看到"地图"——项目分哪几大块、块与块怎么连、哪些连接是分析器猜的——再钻进符号级细节。

**目标**：snapshot 增加 `moduleGraph` 段（模块级聚合图），桌面端增加"模块 / 文件 / 符号"层级切换，dashboard 骨架计数变为可点击入口。

## 2. 现状证据（2026-08-24 实测）

- 真实 snapshot（`webui.snapshot.v1`）：节点仅 `package` / `file` / `symbol` 三种 kind；边仅 `owns` / `defines` / `imports`（Rust 项目另有符号级 `calls`，带 confidence/reason）。
- snapshot 生成端：`scripts/webui-snapshot.sh` 驱动 `scripts/codelattice-snapshot-gen.py`（Python 富化），契约文档 `docs/webui/webui-snapshot-contract.md`。
- 聚合可行性已用 PoC 验证：对 fixture snapshot 做纯数据变换（符号边按文件路径聚合到目录级），产出 3 模块 2 边的模块图，边上保留 count / minConfidence / reason。
- 桌面端 snapshot 消费链：`apps/desktop/src-tauri/src/snapshots.rs`（读 `fixtures/webui-snapshots` / 发布 `target/workbench-snapshots`）→ `snapshot-reader.ts` → G6。

## 3. 设计

### 3.1 模块边界规则（需评审定死）

| 语言 | 模块划分 |
|---|---|
| 全部 | 文件相对路径的前两级目录（去掉文件名）；根目录文件归入 `(root)` |
| Rust 回退 | 仅当节点没有 `file` 时，才用 `Symbol.modulePath` 前两级 `::` |

文件路径优先，是为了对齐验收「模块数 ≈ crates + src 子目录」。`modulePath` 只作无路径时的回退，避免 crate 名把整个仓库收成一块。扁平项目可退化成 1 个模块 / 0 条跨模块边，写入 `limitations.notes`。无路径的**符号**归 `(unknown)`，不做猜测；无路径的 package/file 容器不单独成块，避免空的 `(unknown)` 节点污染架构图。

规则放在 snapshot 生成端，前端不重算（保持"前端有理有据、只渲染事实"）。

### 3.2 `moduleGraph` schema 草案

```json
"moduleGraph": {
  "modules": [{ "id": "src/api", "files": 4, "symbols": 89 }],
  "edges": [{
    "source": "src/components", "target": "src/api",
    "count": 35, "kinds": ["imports"],
    "minConfidence": 0.9,
    "reasons": ["typescript-relative-import-resolved"]
  }],
  "truncated": false
}
```

- `count`：聚合的底层边数；`minConfidence`：聚合边中最低置信度；`reasons`：去重后前 2 条。
- 置信度语义：**minConfidence 是"最弱链路"标注，不是平均值**——前端据此把弱边画虚线，不制造虚假确定感。
- 大项目复用现有 `truncated` 机制：模块数超上限（建议 200）时截断并置 `truncated: true`。

### 3.3 分工三步

1. **snapshot-gen**：`scripts/codelattice-snapshot-gen.py` 增加聚合段产出（纯函数式变换，输入为已生成的 graph 段，不触碰 CALLS 提取逻辑）。
2. **契约**：`docs/webui/webui-snapshot-contract.md` 增加 `moduleGraph` 段定义（optional，缺省为兼容旧 snapshot）；`webui/contract-tests/tests/snapshot-contract.test.mjs` 补校验。
3. **桌面端**：`snapshot-reader.ts` 解析新段；`graph-pane.tsx` 加层级切换 segmented control；`dashboard.tsx` 骨架计数可点击跳转到对应层级。

## 4. Execution Card

**Write set：**

- `scripts/codelattice-snapshot-gen.py`
- `docs/webui/webui-snapshot-contract.md`
- `webui/contract-tests/tests/snapshot-contract.test.mjs`
- `apps/desktop/src/data/snapshot-reader.ts`(+test)
- `apps/desktop/src/panels/graph-pane.tsx`、`dashboard.tsx`、`inspector.tsx`、`model-pool.tsx`
- `apps/desktop/src/App.tsx`、`theme.ts`、`styles.css`、`graph/g6-adapter.ts`、`graph/highlight.ts`
- `fixtures/webui-snapshots/`（写入 `moduleGraph` 基线）

**Forbidden set：**

- `crates/project-model/src/calls.rs` 及任何 CALLS 提取/分类策略（quality watch 生效中，本任务不新增 CALLS 策略）
- `crates/understanding-gateway/`（graph_store 只读消费旧段即可，新段暂不喂 chat 工具）
- `apps/desktop/src-tauri/`（Tauri 壳零改动）
- 旧 `webui/snapshot-viewer/`（P0 stop-line：不回写）

**Stop-line：**

- 模块聚合只做"已有边的向上归并"，**不新造任何符号级边**；聚合后模块边总数 ≤ 底层边总数。
- 若聚合规则需要引入"推断模块归属"（如无路径信息的节点），一律归入 `(unknown)` 并计入 limitations，不做猜测。
- `cargo fmt --check` + `git diff --check` + 相关测试全绿才算完成；提交前跑 `scripts/codelattice-precommit-check.sh`。

## 5. 验收

1. 新跑 fixture snapshot 含 `moduleGraph` 段，contract test 通过。
2. 用 codelattice 自身（Rust 项目）出一份 snapshot：模块数合理（≈ crates 数 + src 子目录级），CALLS 聚合边的 minConfidence/reasons 非空。
3. 桌面端打开该 snapshot：层级切换可用，模块图可点边查看证据，dashboard 骨架计数可点击。
4. 等价性自查：同一 snapshot 的模块边 count 之和 == 底层可聚合边总数（无丢失、无新造）。

## 6. 风险与开放问题

- **模块边界规则的跨语言一致性**：目录划分对扁平结构项目（如 Python 单目录包）会退化成 1 个模块——可接受，但要在 limitations 里说明。
- **性能**：符号级边全量聚合是 O(E)，万级边无压力；前端渲染模块级节点数有上限保护。
- **后续衔接（不在本期）**：detect-changes 结果叠加到模块图（变化入口）、`moduleGraph` 喂给 gateway 的 chat 工具——本期 graph_store 不动，留作下一刀。
