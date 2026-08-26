// GraphController — 图谱画布控制器（P0 §4.1）。
//
// 绑定 G6Adapter 事件 → GraphSelectionStore；树/Inspector 只订阅 store。
// 选择动作只更新本地状态，绝不触发网络/模型请求（验收 2）。
import type { GraphSelection } from "../types";
import type { G6AdapterCallbacks } from "./g6-adapter";
import type { GraphSelectionStore } from "../state/graph-selection";

export class GraphController implements G6AdapterCallbacks {
  constructor(
    private readonly selectionStore: GraphSelectionStore,
    private readonly snapshotId: string,
    private readonly additive: () => boolean = () => false,
  ) {}

  onSelectNode(nodeId: string, shift = false): void {
    const snapshotId = this.currentSnapshotId();
    if (shift || this.additive()) {
      this.selectionStore.dispatch({ type: "toggle-node", nodeId, snapshotId });
      return;
    }
    this.selectionStore.dispatch({ type: "select-node", nodeId, snapshotId });
  }

  onFocusNode(nodeId: string): void {
    this.onSelectNode(nodeId, false);
  }

  onHoverNode(_nodeId: string | null): void {}

  onSelectEdge(relationKey: string, shift = false): void {
    const snapshotId = this.currentSnapshotId();
    if (shift || this.additive()) {
      this.selectionStore.dispatch({ type: "toggle-relation", relationKey, snapshotId });
      return;
    }
    this.selectionStore.dispatch({ type: "select-relation", relationKey, snapshotId });
  }

  onHoverEdge(_relationKey: string | null): void {
    // hover 不改变选择状态
  }

  onCanvasClick(): void {
    this.selectionStore.dispatch({ type: "clear" });
  }

  private currentSnapshotId(): string {
    return this.snapshotId;
  }
}

export function selectionForSnapshot(sel: GraphSelection, snapshotId: string): GraphSelection {
  if (sel.type === "none") return sel;
  return { ...sel, snapshotId };
}
