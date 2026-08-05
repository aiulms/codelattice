// GraphController — 图谱画布控制器（P0 §4.1）。
//
// 绑定 G6Adapter 事件 → GraphSelectionStore；树/Inspector 只订阅 store。
// 选择动作只更新本地状态，绝不触发网络/模型请求（验收 2）。
import type { GraphSelection } from "../types";
import type { G6AdapterCallbacks } from "./g6-adapter";
import type { GraphSelectionStore } from "../state/graph-selection";

export class GraphController implements G6AdapterCallbacks {
  constructor(private readonly selectionStore: GraphSelectionStore) {}

  onSelectNode(nodeId: string): void {
    const snapshotId = this.currentSnapshotId();
    this.selectionStore.dispatch({ type: "select-node", nodeId, snapshotId });
  }

  onFocusNode(nodeId: string): void {
    // dblclick 聚焦：只更新选择，聚焦画布由 UI 层处理
    this.onSelectNode(nodeId);
  }

  onHoverNode(_nodeId: string | null): void {
    // hover 不改变选择状态（P2：解释显式触发）
  }

  onSelectEdge(relationKey: string): void {
    const snapshotId = this.currentSnapshotId();
    this.selectionStore.dispatch({ type: "select-relation", relationKey, snapshotId });
  }

  onHoverEdge(_relationKey: string | null): void {
    // hover 不改变选择状态
  }

  onCanvasClick(): void {
    this.selectionStore.dispatch({ type: "clear" });
  }

  private currentSnapshotId(): string {
    const sel = this.selectionStore.getState();
    return sel.type === "none" ? "" : sel.snapshotId;
  }
}

export function selectionForSnapshot(sel: GraphSelection, snapshotId: string): GraphSelection {
  if (sel.type === "none") return sel;
  return { ...sel, snapshotId };
}
