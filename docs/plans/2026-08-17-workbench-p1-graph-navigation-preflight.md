# Workbench P1 图谱导航 · Preflight（2026-08-17）

隶属：`2026-08-17-graph-quality-roadmap-pack.md` 任务 4。本轮只做 preflight，
不冻结执行卡、不实施。

## 动机（P0 遗留边界）

P0 视觉返工交付了 `d3-force + preventOverlap + autoFit`（`apps/desktop/src/graph/g6-adapter.ts:125-134`），
解决"无坐标 snapshot 全部堆原点"的可读性问题。P0 closure 明确挂账：
d3-force 只保证默认可读，**不覆盖大图的社区折叠、分层布局或语义聚类**
——这些属于 P1 图谱导航能力。

真实规模锚点：
- CodeLattice self-analysis 全图 6981 节点 / 8486 边（2026-08-17 实测）。
- Web snapshot 预览截断为 150 节点 / 300 边（`scripts/codelattice-snapshot-gen.py:624`）。
- 桌面端 QueryStore 持有完整不可变图（8 snapshot / 512MB 上限），
  因此 P1 导航是"完整数据已就位、缺呈现与交互层"的问题，不需要新的
  数据链路。

## 现状锚点

| 层 | 现状 | P1 缺口 |
|---|---|---|
| 布局 | 单一 d3-force，扁平渲染全部节点 | 无层次/分组语义；大图成一团 |
| 交互 | node/edge click、选择联动 Inspector | 无折叠/展开、无子图聚焦 |
| 数据 | QueryStore 全图 + relationKey 边身份 | 无社区/模块聚类预计算 |
| 模型层 | 理解网关只读查询 | 聚类结果属派生视图，不得写回事实（P0 冻结决策） |

## 候选切片（按依赖顺序）

### P1-A：模块分层布局（建议先做）

- 按 snapshot 已有的事实分层：package → source file → symbol，
  用 G5 combo / dagre 分层替代（或叠加）d3-force。
- 数据全部来自现有节点属性（packageId/sourcePath），零新分析。
- 风险最低，且为后续折叠提供分组骨架。

### P1-B：社区折叠与展开

- 基于 CALLS/IMPORTS 边密度做社区检测（label propagation / Louvain，
  纯 Rust 或 WASM，需选型）；社区作为可折叠 combo。
- 折叠状态是 UI 派生状态，只存前端 store，不写 snapshot。
- 需要评估：社区检测在 7k 节点规模的耗时（目标 <1s，可后台算）。

### P1-C：语义聚类（依赖理解网关）

- 用模型解释为社区/模块生成人类可读命名（"缓存系统""解析器管线"）。
- 严格遵守 P0 冻结决策：模型输出进 understanding-cache 会话层，
  不写回事实 snapshot；无模型时社区显示事实性名称（module path）。

## 数据型决策（开工前必须收敛，不能靠实现者猜）

1. 布局引擎选型：G6 v5 内置 combo 布局 vs 自研分层 —— 需 spike 实测
   7k 节点渲染帧率与布局耗时。
2. 社区检测算法与阈值：模块边界（事实）与社区边界（推导）冲突时以
   事实为准；阈值需在 CodeLattice self + 一个外部 fixture 上调定。
3. 大图分级策略：全图渲染上限（超过时自动折叠到 package 层）的数值。
4. P1-B/P1-C 是否合并为一张执行卡（若 P1-A spike 显示 combo 折叠
   天然承载社区，则合并）。

## Stop-lines（继承 P0 + 新增）

- 聚类/折叠/语义命名均为派生视图：不写回 snapshot、不进 Rust 事实层、
  不改变 relationKey 契约。
- 语义命名必须可整体关闭（无模型时功能完整可用，只缺命名）。
- 不引入重型图计算依赖（图神经网络、embedding 服务）；社区检测限于
  经典图算法。
- WKWebView 实测帧率与内存不劣化 P0 F1 基线（aggregate ≤512MB）。
- 旧 `webui/snapshot-viewer` 不动。

## 建议的验证骨架（供执行卡细化）

- 布局确定性 characterization（同 snapshot 两次布局坐标一致）。
- 折叠/展开的状态机测试（前端 store 层，不测 G6 像素）。
- 7k 节点 fixture 的 WKWebView smoke + 内存采样（复用 F1 采样器）。
- 契约测试：折叠状态下 Inspector/Chat 的 evidence 查询仍指向全图
  QueryStore（不是折叠后的子图）。

## 关联待办（非本卡范围）

- `mcp_consistency_review_old_client_stale` 存量失败（find_symbols 子串
  匹配 `testOldClient` 命中 `OldClient`）：typescript-feature 门下长期
  未跑暴露，建议单独小卡修复（匹配策略：测试符号降权或精确名优先）。
