// Graph highlight semantics — 纯函数（P0-F0 字符化基线冻结）。
//
// 与旧 graph-g6.js 行为对齐：选中 node → 自身 + 邻接边高亮、邻接节点
// neighbor、其余 dim；选中 relation → 该边高亮、两端节点 neighbor、其余 dim。
// 输入输出均无副作用、无布局依赖，可独立测试。
import type { GraphSelection, SnapshotEdge, SnapshotNode } from "../types";
import { relationKeyOf } from "../data/snapshot-reader";

export type ElementHighlight = {
  selected: Set<string>;
  neighbor: Set<string>;
  dimmed: Set<string>;
};

/** 计算给定 selection 下的节点/边高亮集合（edge 用 relationKey 标识）。 */
export function computeHighlight(
  nodes: SnapshotNode[],
  edges: SnapshotEdge[],
  selection: GraphSelection,
): ElementHighlight {
  const selected = new Set<string>();
  const neighbor = new Set<string>();
  const dimmed = new Set<string>();
  const nodeIds = new Set(nodes.map((n) => n.id));

  if (selection.type === "none") {
    return { selected, neighbor, dimmed };
  }

  if (selection.type === "node") {
    const sid = selection.nodeId;
    if (!nodeIds.has(sid)) return { selected, neighbor, dimmed };
    selected.add(sid);
    for (const e of edges) {
      const key = relationKeyOf(e);
      if (e.source === sid || e.target === sid) {
        selected.add(key);
        neighbor.add(e.source);
        neighbor.add(e.target);
      }
    }
  } else if (selection.type === "relation") {
    const e = edges.find((x) => relationKeyOf(x) === selection.relationKey);
    if (!e) return { selected, neighbor, dimmed };
    selected.add(selection.relationKey);
    neighbor.add(e.source);
    neighbor.add(e.target);
  } else {
    // chain: 高亮链上所有节点与边（调用方已把 chain 展开为 nodeIds/relationKeys 传入?）
    // 这里保守处理：chain 标识由 controller 转成 node/relation 高亮，见 controller。
    return { selected, neighbor, dimmed };
  }

  for (const id of nodeIds) {
    if (!selected.has(id) && !neighbor.has(id)) dimmed.add(id);
  }
  return { selected, neighbor, dimmed };
}
