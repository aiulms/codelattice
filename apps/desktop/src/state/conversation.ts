// ConversationStore — 独立对话上下文 store（返工第二轮 A-fix）。
//
// 修复：
// - 增加 replace-session action（后端创建 session 后替换前端 sessionId）
// - 增加 close action
// - App 通过 subscribe 将 context 放入 React state
import type { ConversationContext, ConversationScopeType } from "../types";

export type ConversationAction =
  | { type: "replace-session"; sessionId: string; snapshotId: string }
  | { type: "pin"; scopeType: ConversationScopeType; id: string; snapshotId: string }
  | { type: "clear-scope" }
  | { type: "snapshot-changed"; snapshotId: string }
  | { type: "close" };

export function createConversationState(snapshotId: string): ConversationContext {
  return { sessionId: "", pinnedScope: null, snapshotId, stale: false };
}

export function conversationReducer(
  state: ConversationContext,
  action: ConversationAction,
): ConversationContext {
  switch (action.type) {
    case "replace-session":
      return {
        sessionId: action.sessionId,
        pinnedScope: null,
        snapshotId: action.snapshotId,
        stale: false,
      };
    case "pin":
      return {
        ...state,
        pinnedScope: { type: action.scopeType, id: action.id },
        snapshotId: action.snapshotId,
        stale: false,
      };
    case "clear-scope":
      return { ...state, pinnedScope: null };
    case "snapshot-changed":
      // 旧对话保持 pinned 但标记 stale
      return {
        ...state,
        snapshotId: action.snapshotId,
        stale: state.pinnedScope !== null,
      };
    case "close":
      return { sessionId: "", pinnedScope: null, snapshotId: "", stale: false };
  }
}

export type ConversationListener = (ctx: ConversationContext) => void;

export class ConversationStore {
  private state: ConversationContext;
  private listeners = new Set<ConversationListener>();

  constructor(snapshotId: string) {
    this.state = createConversationState(snapshotId);
  }

  getState(): ConversationContext {
    return this.state;
  }

  subscribe(fn: ConversationListener): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  dispatch(action: ConversationAction): ConversationContext {
    const next = conversationReducer(this.state, action);
    if (next !== this.state) {
      this.state = next;
      for (const fn of this.listeners) fn(this.state);
    }
    return this.state;
  }
}
