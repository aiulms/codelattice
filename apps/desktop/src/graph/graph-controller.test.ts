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
});
