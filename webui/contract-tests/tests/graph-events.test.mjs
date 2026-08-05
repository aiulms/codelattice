// P0-F0 characterization: G6 adapter (graph-g6.js) selection semantics and
// interaction event payloads, loaded in a VM with a fake G6/document.
//
// Freezes observable behavior BEFORE the new Workbench reimplements the
// selection contract (GraphSelectionStore). Visual layout/coordinates are
// deliberately NOT characterized (P0-F0 scope boundary).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WS = path.resolve(__dirname, "../../..");
const G6_SRC = path.join(WS, "webui/snapshot-viewer/graph-g6.js");

const FIXTURE_NODES = [
  { id: "n:a", label: "a", kind: "symbol" },
  { id: "n:b", label: "b", kind: "symbol" },
  { id: "n:c", label: "c", kind: "symbol" },
  { id: "n:f1", label: "f1.rs", kind: "file" },
  { id: "n:pkg", label: "pkg", kind: "package" },
];
const FIXTURE_EDGES = [
  { source: "n:a", target: "n:b", kind: "calls", confidence: 0.9, reason: "direct" },
  { source: "n:b", target: "n:c", kind: "calls", confidence: 0.8, reason: "direct" },
  { source: "n:f1", target: "n:a", kind: "defines" },
];

/** Load graph-g6.js in a sandbox and return { api, captured } handles. */
function loadAdapter() {
  assert.ok(existsSync(G6_SRC), `graph-g6.js missing: ${G6_SRC}`);
  const captured = {
    config: null,
    handlers: {},   // event name -> callback
    stateSets: [],  // setElementState argument history
    destroyed: 0,
    appended: [],
  };

  class FakeGraph {
    constructor(config) { captured.config = config; }
    on(evt, fn) { captured.handlers[evt] = fn; }
    render() {
      captured.config.data.nodes.forEach((n) => {
        if (n.style.opacity === 0.18) captured.dimmed = (captured.dimmed || 0) + 1;
      });
      return Promise.resolve();
    }
    setElementState(states) { captured.stateSets.push(states); }
    destroy() { captured.destroyed += 1; }
  }

  const fakeG6 = { Graph: FakeGraph };
  const host = {
    innerHTML: "",
    className: "",
    style: {},
    clientWidth: 1200,
    clientHeight: 800,
    dataset: {},
    appendChild: (el) => { captured.appended.push(el); },
  };
  const fakeDoc = { createElement: () => ({ innerHTML: "", className: "", style: {} }) };
  const windowObj = { G6: fakeG6 };
  const sandbox = {
    window: windowObj,
    document: fakeDoc,
    console,
    Set,
    Promise,
    Math,
    Number,
    String,
  };
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(readFileSync(G6_SRC, "utf8"), sandbox, { filename: "graph-g6.js" });
  return { api: windowObj.CodeLatticeG6Graph, captured, host };
}

test("adapter exposes the documented surface", () => {
  const { api } = loadAdapter();
  assert.equal(api.version, "g6-5.1.1");
  assert.equal(typeof api.render, "function");
  assert.equal(typeof api.select, "function");
  assert.equal(typeof api.destroy, "function");
  assert.equal(typeof api.lastRendered, "function");
});

test("render without selection keeps all nodes/edges visible and unselected", () => {
  const { api, captured, host } = loadAdapter();
  const ok = api.render({ host, nodes: FIXTURE_NODES, edges: FIXTURE_EDGES, layout: "galaxy" });
  assert.equal(ok, true);
  const { nodes, edges } = captured.config.data;
  assert.equal(nodes.length, FIXTURE_NODES.length);
  for (const n of nodes) {
    assert.equal(n.data.selected, false);
    assert.equal(n.data.neighbor, false);
    assert.notEqual(n.style.opacity, 0.18, "no dimming without selection");
  }
  for (const e of edges) assert.equal(e.data.selected, false);
});

test("selection semantics: selected node + incident edges highlighted, others dimmed", () => {
  const { api, captured, host } = loadAdapter();
  api.render({
    host, nodes: FIXTURE_NODES, edges: FIXTURE_EDGES,
    layout: "galaxy", selectedNodeId: "n:b",
  });
  const { nodes, edges } = captured.config.data;
  const byId = Object.fromEntries(nodes.map((n) => [n.id, n]));
  // selected node itself
  assert.equal(byId["n:b"].data.selected, true);
  // direct neighbors via incident edges
  assert.equal(byId["n:a"].data.neighbor, true, "n:a is a neighbor of n:b");
  assert.equal(byId["n:c"].data.neighbor, true, "n:c is a neighbor of n:b");
  // non-neighbors dimmed (opacity 0.18)
  assert.equal(byId["n:f1"].style.opacity, 0.18, "n:f1 not adjacent to n:b -> dimmed");
  assert.equal(byId["n:pkg"].style.opacity, 0.18, "n:pkg not adjacent to n:b -> dimmed");
  // edges incident to selection flagged selected
  const edgeSel = edges.filter((e) => e.data.selected);
  assert.deepEqual(edgeSel.map((e) => `${e.source}->${e.target}`).sort(),
    ["n:a->n:b", "n:b->n:c"].sort());
});

test("edge ids are stable per source-target pair with dedupe suffix for parallel edges", () => {
  const { api, captured, host } = loadAdapter();
  api.render({
    host,
    nodes: FIXTURE_NODES,
    edges: [
      { source: "n:a", target: "n:b", kind: "calls" },
      { source: "n:a", target: "n:b", kind: "calls" },
    ],
  });
  const ids = captured.config.data.edges.map((e) => e.id);
  assert.deepEqual(ids, ["edge-n:a-n:b", "edge-n:a-n:b#1"],
    "parallel edge ids are layout-relative, NOT semantic identities (P0-A gap)");
});

test("event payloads: node click emits id, dblclick focus, hover id+evt, canvas resets", () => {
  const { api, captured, host } = loadAdapter();
  const events = [];
  api.render({
    host, nodes: FIXTURE_NODES, edges: FIXTURE_EDGES,
    onSelect: (id) => events.push(["select", id]),
    onFocus: (id) => events.push(["focus", id]),
    onHover: (id, evt) => events.push(["hover", id, evt]),
    onHoverEnd: () => events.push(["hoverEnd"]),
  });
  const h = captured.handlers;
  // node click: payload is the raw node id only
  h["node:click"]({ target: { id: "n:a" } });
  assert.deepEqual(events[0], ["select", "n:a"]);
  // dblclick -> focus
  h["node:dblclick"]({ target: { id: "n:b" } });
  assert.deepEqual(events[1], ["focus", "n:b"]);
  // pointerenter -> hover with original event passthrough
  // (the adapter resolves the node id from evt.target, then forwards
  // evt.originalEvent || evt.event || evt)
  const origEvt = { target: { id: "n:b" }, originalEvent: { type: "pointerenter" } };
  h["node:pointerenter"](origEvt);
  assert.deepEqual(events[2], ["hover", "n:b", { type: "pointerenter" }]);
  h["node:pointerleave"]();
  assert.deepEqual(events[3], ["hoverEnd"]);
  // canvas click clears all element states
  const before = captured.stateSets.length;
  h["canvas:click"]();
  assert.ok(captured.stateSets.length === before + 1, "canvas click resets element states");
  const cleared = captured.stateSets.at(-1);
  for (const id of FIXTURE_NODES.map((n) => n.id)) {
    // NOTE: cleared[id] is a VM-realm array; use length check (strict
    // deepEqual compares prototypes across realms).
    assert.ok(Array.isArray(cleared[id]) && cleared[id].length === 0,
      `state cleared for node ${id}`);
  }
});

test("P0-A gap: no edge click/hover/selection events in the legacy adapter", () => {
  const { api, captured, host } = loadAdapter();
  api.render({ host, nodes: FIXTURE_NODES, edges: FIXTURE_EDGES });
  const handlerNames = Object.keys(captured.handlers);
  for (const evt of ["edge:click", "edge:dblclick", "edge:pointerenter", "edge:pointerleave"]) {
    assert.ok(!handlerNames.includes(evt), `edge event ${evt} must not exist yet (P0-A adds it)`);
  }
});
