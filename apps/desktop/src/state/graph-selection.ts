// GraphSelectionStore — 独立的选择状态 store（P0 §4.1 / 验收 1–4, 9–10）。
//
// 树、画布和 Inspector 只订阅本 store；Chat 通过显式 NavigationRequest 联动
// （见 navigationRequest()），而不是被对话上下文隐式覆盖。
// 选择节点/边本身绝不允许产生网络或模型请求（验收 2）。
import type { GraphSelection, NavigationAction } from "../types";

export type SelectionAction =
  | { type: "select-node"; nodeId: string; snapshotId: string }
  | { type: "select-relation"; relationKey: string; occurrenceKey?: string; snapshotId: string }
  | { type: "select-chain"; chainId: string; snapshotId: string }
  | { type: "clear" };

export type NavigationResult =
  | { kind: "applied"; selection: GraphSelection }
  | { kind: "rejected"; reason: "invalid-action" | "cross-snapshot" | "unknown-node" | "unknown-relation" };

const EMPTY: GraphSelection = { type: "none" };

/** 纯 reducer：selection transition，可独立测试（F0 字符化基线为语义参考）。 */
export function selectionReducer(state: GraphSelection, action: SelectionAction): GraphSelection {
  switch (action.type) {
    case "select-node":
      // 幂等：重复选择同一节点返回同一引用，避免无意义重渲染
      if (state.type === "node" && state.nodeId === action.nodeId && state.snapshotId === action.snapshotId) {
        return state;
      }
      return { type: "node", nodeId: action.nodeId, snapshotId: action.snapshotId };
    case "select-relation": {
      if (
        state.type === "relation" &&
        state.relationKey === action.relationKey &&
        state.snapshotId === action.snapshotId &&
        (state.occurrenceKey ?? null) === (action.occurrenceKey ?? null)
      ) {
        return state;
      }
      const sel: GraphSelection = {
        type: "relation",
        relationKey: action.relationKey,
        snapshotId: action.snapshotId,
      };
      if (action.occurrenceKey) (sel as { occurrenceKey?: string }).occurrenceKey = action.occurrenceKey;
      return sel;
    }
    case "select-chain":
      if (state.type === "chain" && state.chainId === action.chainId && state.snapshotId === action.snapshotId) {
        return state;
      }
      return { type: "chain", chainId: action.chainId, snapshotId: action.snapshotId };
    case "clear":
      return EMPTY;
  }
}

/**
 * 处理来自 Chat evidence chip 的显式 NavigationRequest（验收 10）。
 * - 非法 action 类型 → rejected
 * - 目标 snapshotId 与当前已加载 snapshot 不一致且未确认 → rejected
 * - node/relation 必须存在于当前 snapshot（由 reader 提供校验器）
 */
export function applyNavigation(
  action: NavigationAction,
  currentSnapshotId: string,
  exists: {
    node: (id: string) => boolean;
    relation: (relationKey: string) => boolean;
  },
  confirmCrossSnapshot: (targetSnapshotId: string) => boolean,
): NavigationResult {
  switch (action.type) {
    case "focusNode": {
      if (action.snapshotId !== currentSnapshotId && !confirmCrossSnapshot(action.snapshotId)) {
        return { kind: "rejected", reason: "cross-snapshot" };
      }
      if (!exists.node(action.nodeId)) return { kind: "rejected", reason: "unknown-node" };
      return {
        kind: "applied",
        selection: { type: "node", nodeId: action.nodeId, snapshotId: action.snapshotId },
      };
    }
    case "focusRelation": {
      if (action.snapshotId !== currentSnapshotId && !confirmCrossSnapshot(action.snapshotId)) {
        return { kind: "rejected", reason: "cross-snapshot" };
      }
      if (!exists.relation(action.relationKey)) return { kind: "rejected", reason: "unknown-relation" };
      const sel: GraphSelection = {
        type: "relation",
        relationKey: action.relationKey,
        snapshotId: action.snapshotId,
      };
      if (action.occurrenceKey) (sel as { occurrenceKey?: string }).occurrenceKey = action.occurrenceKey;
      return { kind: "applied", selection: sel };
    }
    case "focusSource":
      // sourceRef 定位只更新 Inspector 的源码视图，不改变图谱选择；
      // 这里按图谱语义返回 rejected(invalid-action) 由调用方转处理。
      return { kind: "rejected", reason: "invalid-action" };
  }
}

export type SelectionListener = (sel: GraphSelection) => void;

/** 最小可订阅 store；不依赖 React，可在任何环境使用。 */
export class GraphSelectionStore {
  private state: GraphSelection = EMPTY;
  private listeners = new Set<SelectionListener>();

  getState(): GraphSelection {
    return this.state;
  }

  subscribe(fn: SelectionListener): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  dispatch(action: SelectionAction): GraphSelection {
    const next = selectionReducer(this.state, action);
    if (next !== this.state) {
      this.state = next;
      for (const fn of this.listeners) fn(this.state);
    }
    return this.state;
  }

  navigate(
    action: NavigationAction,
    currentSnapshotId: string,
    exists: { node: (id: string) => boolean; relation: (key: string) => boolean },
    confirmCrossSnapshot: (id: string) => boolean,
  ): NavigationResult {
    const result = applyNavigation(action, currentSnapshotId, exists, confirmCrossSnapshot);
    if (result.kind === "applied") {
      const a = action;
      if (a.type === "focusNode") {
        this.dispatch({ type: "select-node", nodeId: a.nodeId, snapshotId: a.snapshotId });
      } else if (a.type === "focusRelation") {
        this.dispatch({
          type: "select-relation",
          relationKey: a.relationKey,
          occurrenceKey: a.occurrenceKey,
          snapshotId: a.snapshotId,
        });
      }
    }
    return result;
  }
}
