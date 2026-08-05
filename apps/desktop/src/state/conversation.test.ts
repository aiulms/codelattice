// G1 gate: ConversationStore 双 store 测试（验收 9）。
import { describe, it, expect } from "vitest";
import { ConversationStore, conversationReducer } from "./conversation";

describe("ConversationStore", () => {
  it("starts with an empty pinned scope and a session id", () => {
    const store = new ConversationStore("snap:1");
    const s = store.getState();
    expect(s.pinnedScope).toBeNull();
    expect(s.snapshotId).toBe("snap:1");
    expect(s.stale).toBe(false);
    expect(s.sessionId.startsWith("sess:")).toBe(true);
  });

  it("pin() binds the conversation to the explicit scope (验收 9)", () => {
    const store = new ConversationStore("snap:1");
    store.dispatch({ type: "pin", scopeType: "node", id: "n:b", snapshotId: "snap:1" });
    expect(store.getState().pinnedScope).toEqual({ type: "node", id: "n:b" });
  });

  it("graph selection changes do NOT touch the conversation scope", () => {
    const store = new ConversationStore("snap:1");
    store.dispatch({ type: "pin", scopeType: "node", id: "n:b", snapshotId: "snap:1" });
    // 用户在图谱里选择 A —— ConversationContext 保持 pinned 到 B
    expect(store.getState().pinnedScope).toEqual({ type: "node", id: "n:b" });
  });

  it("snapshot change marks the old conversation stale instead of rebinding", () => {
    const store = new ConversationStore("snap:1");
    store.dispatch({ type: "pin", scopeType: "node", id: "n:b", snapshotId: "snap:1" });
    store.dispatch({ type: "snapshot-changed", snapshotId: "snap:2" });
    const s = store.getState();
    expect(s.stale).toBe(true);
    expect(s.pinnedScope).toEqual({ type: "node", id: "n:b" }); // pinned scope preserved
    expect(s.snapshotId).toBe("snap:2");
  });

  it("clear-scope unpins without changing snapshot binding", () => {
    const store = new ConversationStore("snap:1");
    store.dispatch({ type: "pin", scopeType: "edge", id: "rel:ab", snapshotId: "snap:1" });
    store.dispatch({ type: "clear-scope" });
    expect(store.getState().pinnedScope).toBeNull();
  });
});

describe("conversationReducer purity", () => {
  it("is a pure function over (state, action)", () => {
    const s1 = conversationReducer(
      { sessionId: "s", pinnedScope: null, snapshotId: "snap:1", stale: false },
      { type: "pin", scopeType: "project", id: "p", snapshotId: "snap:1" },
    );
    const s2 = conversationReducer(
      { sessionId: "s", pinnedScope: null, snapshotId: "snap:1", stale: false },
      { type: "pin", scopeType: "project", id: "p", snapshotId: "snap:1" },
    );
    expect(s1).toEqual(s2);
  });
});
