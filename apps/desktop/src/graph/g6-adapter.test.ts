// G6GraphAdapter 契约测试：edge 事件 + relationKey 元素身份（P0-A #2 的前置，
// F1 即建立；F0 字符化确认 legacy 无 edge 事件 —— 这里是新增契约）。
import { describe, it, expect } from "vitest";
import { G6GraphAdapter } from "./g6-adapter";
import type { G6GraphLike } from "./g6-adapter";
import type { GraphSelection, SnapshotEdge, SnapshotNode } from "../types";
import { defaultRelationKey } from "../data/snapshot-reader";

const nodes: SnapshotNode[] = [
  { id: "n:a", label: "a", kind: "symbol" },
  { id: "n:b", label: "b", kind: "symbol" },
  { id: "n:c", label: "c", kind: "symbol" },
];
const edges: SnapshotEdge[] = [
  { source: "n:a", target: "n:b", kind: "calls" },
  { source: "n:b", target: "n:c", kind: "calls" },
];

type Handler = (evt: { target?: { id?: string }; originalEvent?: unknown; targetType?: string }) => void;

function makeFake() {
  const state: {
    config: Record<string, unknown> | null;
    handlers: Map<string, Handler>;
    destroyed: boolean;
    focus: string | string[] | null;
  } = { config: null, handlers: new Map(), destroyed: false, focus: null };
  class FakeGraph implements G6GraphLike {
    constructor(config: unknown) { state.config = config as Record<string, unknown>; }
    on(event: string, fn: Handler) { state.handlers.set(event, fn); }
    render() { return Promise.resolve(); }
    destroy() { state.destroyed = true; }
    setElementState() { /* noop */ }
    focusElement(id: string | string[]) { state.focus = id; }
    emit(event: string, evt: unknown) {
      const h = state.handlers.get(event);
      if (h) h(evt as { target?: { id?: string } });
    }
  }
  return { FakeGraph, state };
}

function host(): HTMLElement {
  return { clientWidth: 800, clientHeight: 600 } as HTMLElement;
}

describe("G6GraphAdapter edge contract (F1 新增，P0-A #2 前置)", () => {
  it("applies a collision-aware force layout and fits the graph into the viewport", () => {
    const { FakeGraph, state } = makeFake();
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );

    adapter.render(nodes, edges, {});

    expect(state.config).toMatchObject({
      autoFit: "view",
      layout: {
        type: "d3-force",
        preventOverlap: true,
      },
    });
  });

  it("edge elements use unique instance ids mapping to relationKey", () => {
    const { FakeGraph, state } = makeFake();
    const events: string[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: (k) => events.push(`edge:${k}`), onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    const g6Edges = (state.config as { data: { edges: Array<{ id: string }> } }).data.edges;
    // §6.1：元素 id 是唯一实例身份（relationKey#序号），不是纯 relationKey
    expect(g6Edges[0].id).toBe(`${defaultRelationKey("n:a", "calls", "n:b")}#0`);
    expect(g6Edges[1].id).toBe(`${defaultRelationKey("n:b", "calls", "n:c")}#1`);
  });

  it("edge:click emits relationKey through the callback (not layout id)", () => {
    const { FakeGraph, state } = makeFake();
    const events: string[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: (k) => events.push(k), onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    const key = defaultRelationKey("n:a", "calls", "n:b");
    (state.handlers.get("edge:click") as Handler)({ target: { id: `${key}#0` } });
    expect(events).toEqual([key]);
  });

  it("shift-click marks the edge selection as additive", () => {
    const { FakeGraph, state } = makeFake();
    const flags: boolean[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: (_k, add) => flags.push(!!add), onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    const key = defaultRelationKey("n:a", "calls", "n:b");
    (state.handlers.get("edge:click") as Handler)({
      target: { id: `${key}#0` },
      originalEvent: { shiftKey: true },
    });
    expect(flags).toEqual([true]);
  });

  it("parallel edges (same source/kind/target) render with unique ids and map to one relationKey", () => {
    // G2 关键场景：真实 snapshot（shell/typescript）存在平行边；relationKey 相同但
    // G6 元素 id 必须唯一，否则 WKWebView smoke 报 "Edge already exists"（G1 复现根因）。
    const { FakeGraph, state } = makeFake();
    const events: string[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: (k) => events.push(k), onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    const parallel: SnapshotEdge[] = [
      { source: "n:a", target: "n:b", kind: "calls" },
      { source: "n:a", target: "n:b", kind: "calls" },
      { source: "n:a", target: "n:b", kind: "calls" },
    ];
    expect(adapter.render(nodes, parallel, {})).toBe(true);
    const g6Edges = (state.config as { data: { edges: Array<{ id: string }> } }).data.edges;
    const ids = g6Edges.map((e) => e.id);
    expect(new Set(ids).size).toBe(3);
    const key = defaultRelationKey("n:a", "calls", "n:b");
    // 平行边元素全部映射回同一语义 relationKey
    const mapped = adapter.edgeElementIds().get(key);
    expect(mapped?.sort()).toEqual([...ids].sort());
    // 点击任一平行边元素都发出同一 relationKey（relation-level 语义）
    (state.handlers.get("edge:click") as Handler)({ target: { id: ids[2] } });
    expect(events).toEqual([key]);
  });

  it("node:click / canvas:click wiring works", () => {
    const { FakeGraph, state } = makeFake();
    const events: string[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: (id) => events.push(`node:${id}`), onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => events.push("canvas") },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    (state.handlers.get("node:click") as Handler)({ target: { id: "n:a" } });
    (state.handlers.get("canvas:click") as Handler)({});
    expect(events).toEqual(["node:n:a", "canvas"]);
  });

  it("canvas:click on an edge target does not clear selection", () => {
    const { FakeGraph, state } = makeFake();
    const events: string[] = [];
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => events.push("canvas") },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    (state.handlers.get("canvas:click") as Handler)({ targetType: "edge" });
    (state.handlers.get("canvas:click") as Handler)({ targetType: "node" });
    expect(events).toEqual([]);
  });

  it("destroy cleans up and render can remount (F1 #7 重复 mount/unmount)", () => {
    const { FakeGraph, state } = makeFake();
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    expect(adapter.render(nodes, edges, {})).toBe(true);
    adapter.destroy();
    expect(state.destroyed).toBe(true);
    // remount
    expect(adapter.render(nodes, edges, {})).toBe(true);
  });

  it("selection update does not remount the graph", () => {
    const { FakeGraph, state } = makeFake();
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    const firstConfig = state.config;
    state.destroyed = false;
    adapter.setSelection({ type: "node", nodeId: "n:b", snapshotId: "s" });
    expect(state.destroyed).toBe(false);
    expect(state.config).toBe(firstConfig);
  });

  it("selection renders highlight flags into elements", () => {
    const { FakeGraph, state } = makeFake();
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    const sel: GraphSelection = { type: "node", nodeId: "n:b", snapshotId: "s" };
    adapter.render(nodes, edges, { selection: sel });
    const g6Nodes = (state.config as { data: { nodes: Array<{ id: string; data: { selected: boolean; neighbor: boolean } }> } }).data.nodes;
    const nb = g6Nodes.find((n) => n.id === "n:b");
    expect(nb?.data.selected).toBe(true);
    expect(g6Nodes.find((n) => n.id === "n:a")?.data.neighbor).toBe(true);
  });

  it("frames the camera on the selected edge endpoints", () => {
    const { FakeGraph, state } = makeFake();
    const adapter = new G6GraphAdapter(
      { onSelectNode: () => {}, onFocusNode: () => {}, onHoverNode: () => {},
        onSelectEdge: () => {}, onHoverEdge: () => {}, onCanvasClick: () => {} },
      FakeGraph,
      host(),
    );
    adapter.render(nodes, edges, {});
    const key = defaultRelationKey("n:a", "calls", "n:b");
    adapter.setSelection({ type: "relation", relationKey: key, snapshotId: "s" });
    expect(state.focus).toEqual(["n:a", "n:b"]);
  });
});
