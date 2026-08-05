// E2E: Session lifecycle — 验证 create → pin → close 和 snapshot 变化语义。
import { describe, it, expect } from "vitest";
import { ConversationStore } from "../state/conversation";
import type { ConversationContext } from "../types";

describe("Session lifecycle E2E", () => {
  it("full lifecycle: create → pin → snapshot-changed → close", () => {
    const store = new ConversationStore("snap:1");

    // Initial state: empty session
    expect(store.getState().sessionId).toBe("");
    expect(store.getState().pinnedScope).toBeNull();

    // Backend assigns session via replace-session
    store.dispatch({ type: "replace-session", sessionId: "sess:abc:1", snapshotId: "snap:1" });
    expect(store.getState().sessionId).toBe("sess:abc:1");
    expect(store.getState().stale).toBe(false);

    // Pin scope
    store.dispatch({ type: "pin", scopeType: "node", id: "n:main", snapshotId: "snap:1" });
    expect(store.getState().pinnedScope).toEqual({ type: "node", id: "n:main" });
    expect(store.getState().stale).toBe(false);

    // Snapshot change marks stale
    store.dispatch({ type: "snapshot-changed", snapshotId: "snap:2" });
    expect(store.getState().stale).toBe(true);
    expect(store.getState().pinnedScope).toEqual({ type: "node", id: "n:main" });
    expect(store.getState().snapshotId).toBe("snap:2");

    // Close session
    store.dispatch({ type: "close" });
    expect(store.getState().sessionId).toBe("");
    expect(store.getState().pinnedScope).toBeNull();
    expect(store.getState().snapshotId).toBe("");
  });

  it("multiple sessions: subscribe receives updates", () => {
    const store = new ConversationStore("snap:0");
    const updates: ConversationContext[] = [];
    const unsub = store.subscribe((ctx) => updates.push({ ...ctx }));

    store.dispatch({ type: "replace-session", sessionId: "sess:1", snapshotId: "snap:0" });
    store.dispatch({ type: "pin", scopeType: "project", id: "p", snapshotId: "snap:0" });

    expect(updates.length).toBe(2);
    expect(updates[0].sessionId).toBe("sess:1");
    expect(updates[1].pinnedScope).toEqual({ type: "project", id: "p" });

    unsub();
    store.dispatch({ type: "close" });
    expect(updates.length).toBe(2); // no more updates after unsub
  });

  it("replace-session resets scope (no stale scope from old session)", () => {
    const store = new ConversationStore("snap:1");
    store.dispatch({ type: "replace-session", sessionId: "sess:old", snapshotId: "snap:1" });
    store.dispatch({ type: "pin", scopeType: "node", id: "n:old", snapshotId: "snap:1" });

    // New session should reset pinned scope
    store.dispatch({ type: "replace-session", sessionId: "sess:new", snapshotId: "snap:2" });
    expect(store.getState().sessionId).toBe("sess:new");
    expect(store.getState().pinnedScope).toBeNull();
    expect(store.getState().stale).toBe(false);
    expect(store.getState().snapshotId).toBe("snap:2");
  });
});
