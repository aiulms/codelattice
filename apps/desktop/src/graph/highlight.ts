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
  hovered: Set<string>;
};

export type HighlightHover =
  | { type: "node"; id: string }
  | { type: "edge"; id: string }
  | null;

/** 计算给定 selection 下的节点/边高亮集合（edge 用 relationKey 标识）。 */
export function computeHighlight(
  nodes: SnapshotNode[],
  edges: SnapshotEdge[],
  selection: GraphSelection,
  hover: HighlightHover = null,
): ElementHighlight {
  const selected = new Set<string>();
  const neighbor = new Set<string>();
  const dimmed = new Set<string>();
  const hovered = new Set<string>();
  const nodeIds = new Set(nodes.map((n) => n.id));

  for (const focus of selectionToFoci(selection)) {
    applyFocus(selected, neighbor, nodeIds, edges, focus);
  }
  if (hover) {
    hovered.add(hover.id);
    // 无选中时 hover 充当预览；有选中时只点亮悬停对象，不替换选择
    if (selection.type === "none") {
      applyFocus(
        selected,
        neighbor,
        nodeIds,
        edges,
        hover.type === "node"
          ? { type: "node", nodeId: hover.id, incident: true }
          : { type: "relation", relationKey: hover.id },
      );
    }
  }

  if (selected.size === 0 && hovered.size === 0) {
    return { selected, neighbor, dimmed, hovered };
  }

  for (const id of nodeIds) {
    if (!selected.has(id) && !neighbor.has(id) && !hovered.has(id)) dimmed.add(id);
  }
  // 未选中的边一并变淡，避免「点一下只变色、周围线还在抢视线」
  for (const e of edges) {
    const key = relationKeyOf(e);
    if (!selected.has(key) && !hovered.has(key)) dimmed.add(key);
  }
  return { selected, neighbor, dimmed, hovered };
}

function selectionToFoci(selection: GraphSelection): Focus[] {
  if (selection.type === "node") return [{ type: "node", nodeId: selection.nodeId, incident: true }];
  if (selection.type === "relation") return [{ type: "relation", relationKey: selection.relationKey }];
  if (selection.type === "multi") {
    return [
      ...selection.nodeIds.map((nodeId) => ({ type: "node" as const, nodeId, incident: false })),
      ...selection.relationKeys.map((relationKey) => ({ type: "relation" as const, relationKey })),
    ];
  }
  return [];
}

type Focus =
  | { type: "node"; nodeId: string; incident: boolean }
  | { type: "relation"; relationKey: string };

function applyFocus(
  selected: Set<string>,
  neighbor: Set<string>,
  nodeIds: Set<string>,
  edges: SnapshotEdge[],
  focus: Focus,
): void {
  if (focus.type === "node") {
    if (!nodeIds.has(focus.nodeId)) return;
    selected.add(focus.nodeId);
    if (!focus.incident) return;
    for (const e of edges) {
      const key = relationKeyOf(e);
      if (e.source === focus.nodeId || e.target === focus.nodeId) {
        selected.add(key);
        neighbor.add(e.source);
        neighbor.add(e.target);
      }
    }
    return;
  }
  const e = edges.find((x) => relationKeyOf(x) === focus.relationKey);
  if (!e) return;
  selected.add(focus.relationKey);
  neighbor.add(e.source);
  neighbor.add(e.target);
}
