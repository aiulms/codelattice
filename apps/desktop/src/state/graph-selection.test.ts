// G1 gate: GraphSelectionStore 双 store 测试（验收 2, 9, 10）。
import { describe, it, expect } from "vitest";
import {
  GraphSelectionStore,
  selectionReducer,
  applyNavigation,
} from "./graph-selection";

const snap = "snap:1";
const exists = {
  node: (id: string) => ["n:a", "n:b"].includes(id),
  relation: (key: string) => ["rel:ab", "rel:bc"].includes(key),
};
const neverConfirm = () => false;

describe("selectionReducer", () => {
  it("selects a node with its snapshotId", () => {
    const s = selectionReducer({ type: "none" }, { type: "select-node", nodeId: "n:a", snapshotId: snap });
    expect(s).toEqual({ type: "node", nodeId: "n:a", snapshotId: snap });
  });

  it("selects a relation with optional occurrenceKey", () => {
    const s = selectionReducer(
      { type: "none" },
      { type: "select-relation", relationKey: "rel:ab", occurrenceKey: "occ:1", snapshotId: snap },
    );
    expect(s).toEqual({ type: "relation", relationKey: "rel:ab", occurrenceKey: "occ:1", snapshotId: snap });
  });

  it("keeps relation-level when no occurrenceKey is available", () => {
    const s = selectionReducer(
      { type: "none" },
      { type: "select-relation", relationKey: "rel:ab", snapshotId: snap },
    );
    expect(s.type).toBe("relation");
    expect((s as { occurrenceKey?: string }).occurrenceKey).toBeUndefined();
  });

  it("clears back to none", () => {
    const s = selectionReducer({ type: "node", nodeId: "n:a", snapshotId: snap }, { type: "clear" });
    expect(s).toEqual({ type: "none" });
  });
});

describe("applyNavigation (evidence chip → NavigationRequest, 验收 10)", () => {
  it("applies a valid focusNode for the current snapshot", () => {
    const r = applyNavigation(
      { type: "focusNode", nodeId: "n:a", snapshotId: snap },
      snap, exists, neverConfirm,
    );
    expect(r.kind).toBe("applied");
  });

  it("rejects an unknown node id", () => {
    const r = applyNavigation(
      { type: "focusNode", nodeId: "n:ghost", snapshotId: snap },
      snap, exists, neverConfirm,
    );
    expect(r).toEqual({ kind: "rejected", reason: "unknown-node" });
  });

  it("rejects cross-snapshot navigation without confirmation", () => {
    const r = applyNavigation(
      { type: "focusNode", nodeId: "n:a", snapshotId: "snap:2" },
      snap, exists, neverConfirm,
    );
    expect(r).toEqual({ kind: "rejected", reason: "cross-snapshot" });
  });

  it("accepts cross-snapshot navigation when user confirms", () => {
    const r = applyNavigation(
      { type: "focusNode", nodeId: "n:a", snapshotId: "snap:2" },
      snap, exists, () => true,
    );
    expect(r.kind).toBe("applied");
  });

  it("rejects focusSource as invalid graph navigation (handled by inspector separately)", () => {
    const r = applyNavigation(
      { type: "focusSource", sourceRefId: "src:1", snapshotId: snap },
      snap, exists, neverConfirm,
    );
    expect(r.kind).toBe("rejected");
  });
});

describe("GraphSelectionStore", () => {
  it("notifies subscribers on transition and is pure (no network side effects by construction)", () => {
    const store = new GraphSelectionStore();
    const seen: unknown[] = [];
    store.subscribe((s) => seen.push(s.type));
    store.dispatch({ type: "select-node", nodeId: "n:a", snapshotId: snap });
    store.dispatch({ type: "select-relation", relationKey: "rel:ab", snapshotId: snap });
    expect(seen).toEqual(["node", "relation"]);
    expect(store.getState()).toEqual({ type: "relation", relationKey: "rel:ab", snapshotId: snap });
  });

  it("navigate() applies and dispatches a valid focusRelation", () => {
    const store = new GraphSelectionStore();
    const r = store.navigate(
      { type: "focusRelation", relationKey: "rel:ab", snapshotId: snap },
      snap, exists, neverConfirm,
    );
    expect(r.kind).toBe("applied");
    expect(store.getState().type).toBe("relation");
  });

  it("navigate() leaves state untouched when rejected", () => {
    const store = new GraphSelectionStore();
    store.dispatch({ type: "select-node", nodeId: "n:a", snapshotId: snap });
    const r = store.navigate(
      { type: "focusNode", nodeId: "n:ghost", snapshotId: snap },
      snap, exists, neverConfirm,
    );
    expect(r.kind).toBe("rejected");
    expect(store.getState()).toEqual({ type: "node", nodeId: "n:a", snapshotId: snap });
  });
});
