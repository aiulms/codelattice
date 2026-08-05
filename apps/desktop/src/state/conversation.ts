// ConversationStore — 独立对话上下文 store（P0 §4.1 / 验收 9–10）。
//
// Chat 有自己的对话上下文：用户在图谱选择节点 A 时，Chat 仍 pinned 在 B；
// 只有显式“加入对话”动作（pinScope）才改变 pinned scope。
// snapshotId 改变时旧对话标记 stale，不自动重绑定到新图（验收 10）。
import type { ConversationContext, ConversationScopeType } from "../types";

export type ConversationAction =
  | { type: "pin"; scopeType: ConversationScopeType; id: string; snapshotId: string }
  | { type: "clear-scope" }
  | { type: "snapshot-changed"; snapshotId: string };

export function createConversationState(snapshotId: string): ConversationContext {
  return { sessionId: newSessionId(), pinnedScope: null, snapshotId, stale: false };
}

let sessionCounter = 0;
export function newSessionId(): string {
  sessionCounter += 1;
  return `sess:${Date.now().toString(36)}:${sessionCounter}`;
}

export function conversationReducer(
  state: ConversationContext,
  action: ConversationAction,
): ConversationContext {
  switch (action.type) {
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
      // 旧对话保持 pinned 但标记 stale；不自动重绑定。
      return {
        ...state,
        snapshotId: action.snapshotId,
        stale: state.pinnedScope !== null,
      };
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
