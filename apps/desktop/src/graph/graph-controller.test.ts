import { describe, expect, it } from "vitest";
import { GraphController } from "./graph-controller";
import { GraphSelectionStore } from "../state/graph-selection";

describe("GraphController snapshot identity", () => {
  it("a graph-first click uses the loaded library snapshot id", () => {
    const store = new GraphSelectionStore();
    const controller = new GraphController(store, "rust-portable-smoke.snapshot");

    controller.onSelectNode("n:first");

    expect(store.getState()).toEqual({
      type: "node",
      nodeId: "n:first",
      snapshotId: "rust-portable-smoke.snapshot",
    });
  });

  it("additive edge clicks accumulate into multi", () => {
    const store = new GraphSelectionStore();
    const controller = new GraphController(store, "snap", () => true);
    controller.onSelectEdge("rel:a");
    controller.onSelectEdge("rel:b");
    expect(store.getState()).toEqual({
      type: "multi",
      snapshotId: "snap",
      nodeIds: [],
      relationKeys: ["rel:a", "rel:b"],
    });
  });

  it("shift-click is additive even when the mode callback is off", () => {
    const store = new GraphSelectionStore();
    const controller = new GraphController(store, "snap", () => false);
    controller.onSelectEdge("rel:a");
    controller.onSelectEdge("rel:b", true);
    expect(store.getState()).toEqual({
      type: "multi",
      snapshotId: "snap",
      nodeIds: [],
      relationKeys: ["rel:a", "rel:b"],
    });
  });
});
