/* CodeLattice WebUI - AntV G6 graph engine adapter.
 *
 * G6 is used as a rendering/interaction layer only. CodeLattice keeps graph
 * semantics, ranking, filtering, focus, and fallback ownership in app.js.
 */
(function () {
  "use strict";

  var runtime = {
    graph: null,
    nodeIds: [],
    edgeIds: [],
    lastHost: null,
    lastRendered: false
  };

  function clamp(v, min, max) { return Math.max(min, Math.min(max, v)); }
  function esc(s) {
    return String(s == null ? "" : s).replace(/[&<>"']/g, function (c) {
      return {"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c];
    });
  }
  function G6() { return window.G6 || window.g6; }
  function degreeMap(edges) {
    var degree = {};
    (edges || []).forEach(function (e) {
      degree[e.source] = (degree[e.source] || 0) + 1;
      degree[e.target] = (degree[e.target] || 0) + 1;
    });
    return degree;
  }
  function colorSet(layout) {
    var sets = {
      galaxy: {bg:"#071225", text:"#e0f2fe", symbol:"#38bdf8", file:"#8b5cf6", package:"#fb923c", entry:"#34d399", risk:"#fb7185", edge:"#38bdf8"},
      communities: {bg:"#081c16", text:"#dcfce7", symbol:"#86efac", file:"#38bdf8", package:"#fde047", entry:"#c084fc", risk:"#fb7185", edge:"#22c55e"},
      flow: {bg:"#08111f", text:"#dbeafe", symbol:"#93c5fd", file:"#facc15", package:"#5eead4", entry:"#c4b5fd", risk:"#fda4af", edge:"#60a5fa"},
      blueprint: {bg:"#06142f", text:"#dbeafe", symbol:"#fde047", file:"#93c5fd", package:"#fb7185", entry:"#4ade80", risk:"#f97316", edge:"#facc15"},
      orbit: {bg:"#f8fbff", text:"#172033", symbol:"#60a5fa", file:"#34d399", package:"#f59e0b", entry:"#8b5cf6", risk:"#ef4444", edge:"#3b82f6"}
    };
    return sets[layout] || sets.galaxy;
  }
  function nodeColor(n, colors) { return colors[n.kind] || "#94a3b8"; }
  function edgeColor(e, colors) {
    if (e.kind === "calls") return "#f97316";
    if (e.kind === "defines") return colors.file || "#3b82f6";
    if (e.kind === "owns") return "#94a3b8";
    if (e.kind === "imports") return "#10b981";
    return colors.edge || "#64748b";
  }
  function labelText(n) {
    var s = String(n.label || n.id || "");
    if (s.length > 30) return s.slice(0, 27) + "...";
    return s;
  }
  function rankNodes(nodes, edges, limit, focusId) {
    var degree = degreeMap(edges);
    var priority = {package: 0, entry: 1, risk: 2, file: 3, symbol: 4};
    return nodes.slice().sort(function (a, b) {
      if (a.id === focusId) return -1;
      if (b.id === focusId) return 1;
      var pa = priority[a.kind] == null ? 9 : priority[a.kind];
      var pb = priority[b.kind] == null ? 9 : priority[b.kind];
      if (pa !== pb) return pa - pb;
      return (degree[b.id] || 0) - (degree[a.id] || 0);
    }).slice(0, limit);
  }
  function uniqueNodes(nodes) {
    var seen = new Set();
    return (nodes || []).filter(function (n) {
      if (!n || !n.id || seen.has(n.id)) return false;
      seen.add(n.id);
      return true;
    });
  }
  function computePositions(nodes, edges, layout, width, height) {
    var degree = degreeMap(edges);
    var cx = width / 2, cy = height / 2;
    var pos = {};
    var packages = nodes.filter(function (n) { return n.kind === "package"; });
    var files = nodes.filter(function (n) { return n.kind === "file"; });
    var symbols = nodes.filter(function (n) { return n.kind === "symbol"; });
    var others = nodes.filter(function (n) { return n.kind !== "package" && n.kind !== "file" && n.kind !== "symbol"; });
    var fileForSymbol = {};
    edges.forEach(function (e) {
      var s = nodes.find(function (n) { return n.id === e.source; });
      var t = nodes.find(function (n) { return n.id === e.target; });
      if (!s || !t) return;
      if (e.kind === "defines" && s.kind === "file" && t.kind === "symbol") fileForSymbol[t.id] = s.id;
    });
    var symbolsByFile = {};
    symbols.forEach(function (n) {
      var key = fileForSymbol[n.id] || "_orphan";
      (symbolsByFile[key] = symbolsByFile[key] || []).push(n);
    });
    function ring(list, rx, ry, phase, center) {
      center = center || {x: cx, y: cy};
      list.forEach(function (n, i) {
        var a = (Math.PI * 2 * i / Math.max(1, list.length)) + (phase || 0);
        pos[n.id] = {x: clamp(center.x + Math.cos(a) * rx, 42, width - 42), y: clamp(center.y + Math.sin(a) * ry, 38, height - 38)};
      });
    }
    function layer(list, x, top, bottom) {
      list.forEach(function (n, i) {
        var k = list.length <= 1 ? 0.5 : i / (list.length - 1);
        pos[n.id] = {x: x, y: top + k * (bottom - top)};
      });
    }
    function topFiles(limit) {
      return files.slice().sort(function (a, b) {
        return (symbolsByFile[b.id] || []).length - (symbolsByFile[a.id] || []).length || (degree[b.id] || 0) - (degree[a.id] || 0);
      }).slice(0, limit);
    }
    if (layout === "flow" || layout === "blueprint") {
      layer(packages, width * 0.12, height * 0.18, height * 0.82);
      layer(files, width * 0.38, height * 0.10, height * 0.90);
      layer(symbols, width * 0.72, height * 0.08, height * 0.92);
      layer(others, width * 0.90, height * 0.18, height * 0.82);
      if (layout === "blueprint") {
        Object.keys(pos).forEach(function (id, i) {
          pos[id].x = clamp(pos[id].x + ((i % 3) - 1) * 20, 36, width - 36);
          pos[id].y = clamp(pos[id].y + ((i % 5) - 2) * 8, 36, height - 36);
        });
      }
    } else if (layout === "communities") {
      var hubs = topFiles(Math.min(12, Math.max(4, files.length)));
      var cols = Math.ceil(Math.sqrt(Math.max(1, hubs.length)));
      hubs.forEach(function (n, i) {
        var col = i % cols, row = Math.floor(i / cols);
        pos[n.id] = {x: width * (0.15 + (col / Math.max(1, cols - 1)) * 0.70), y: height * (0.22 + (row / Math.max(1, Math.ceil(hubs.length / cols) - 1)) * 0.55)};
      });
      files.filter(function (n) { return !pos[n.id]; }).forEach(function (n, i) {
        var a = Math.PI * 2 * i / Math.max(1, files.length);
        pos[n.id] = {x: cx + Math.cos(a) * width * 0.44, y: cy + Math.sin(a) * height * 0.38};
      });
      Object.keys(symbolsByFile).forEach(function (fid, idx) {
        var anchor = pos[fid] || {x: cx, y: cy};
        var group = symbolsByFile[fid];
        ring(group, 42 + Math.min(110, group.length * 2.2), 36 + Math.min(90, group.length * 1.8), idx * 0.31, anchor);
      });
      ring(packages.concat(others), width * 0.40, height * 0.35, 0);
    } else if (layout === "orbit") {
      ring(packages, 46, 38, -Math.PI / 2);
      ring(files, width * 0.32, height * 0.30, -Math.PI / 2);
      Object.keys(symbolsByFile).forEach(function (fid, idx) {
        var group = symbolsByFile[fid], anchor = pos[fid] || {x: cx, y: cy};
        ring(group, 48 + Math.min(90, group.length * 2.2), 40 + Math.min(74, group.length * 1.8), idx * 0.37, anchor);
      });
      ring(others, width * 0.40, height * 0.36, 0);
    } else {
      packages.forEach(function (n, i) { pos[n.id] = {x: cx + (i - packages.length / 2) * 42, y: cy}; });
      var galaxyHubs = topFiles(Math.min(20, Math.max(8, files.length)));
      var hubSet = new Set(galaxyHubs.map(function (n) { return n.id; }));
      ring(galaxyHubs, width * 0.38, height * 0.34, -Math.PI / 2);
      files.filter(function (n) { return !hubSet.has(n.id); }).forEach(function (n, i) {
        var a = (Math.PI * 2 * i / Math.max(1, files.length)) + 0.2;
        pos[n.id] = {x: cx + Math.cos(a) * width * 0.46, y: cy + Math.sin(a) * height * 0.40};
      });
      Object.keys(symbolsByFile).forEach(function (fid, idx) {
        var anchor = pos[fid] || {x: cx, y: cy};
        var group = symbolsByFile[fid].sort(function (a, b) { return (degree[b.id] || 0) - (degree[a.id] || 0); });
        group.forEach(function (n, i) {
          var a = (Math.PI * 2 * i / Math.max(1, group.length)) + idx * 0.23;
          var r = 38 + Math.min(128, Math.sqrt(group.length) * 15 + i * 0.9);
          pos[n.id] = {x: clamp(anchor.x + Math.cos(a) * r, 34, width - 34), y: clamp(anchor.y + Math.sin(a) * r, 32, height - 32)};
        });
      });
      ring(others, width * 0.43, height * 0.38, Math.PI / 4);
    }
    return pos;
  }
  function sizeFor(n, degree, layout) {
    var d = degree[n.id] || 0;
    var base = n.kind === "package" ? 34 : n.kind === "file" ? 22 : n.kind === "symbol" ? 12 : 16;
    if (layout === "galaxy" || layout === "communities") base += Math.min(24, Math.sqrt(d) * 4.2);
    if (layout === "flow" || layout === "blueprint") base += Math.min(12, Math.sqrt(d) * 2.2);
    return clamp(base, 10, 60);
  }
  function clear() {
    if (runtime.graph && runtime.graph.destroy) {
      try { runtime.graph.destroy(); } catch (_) {}
    }
    runtime.graph = null;
    runtime.nodeIds = [];
    runtime.edgeIds = [];
    runtime.lastRendered = false;
  }
  function eventNodeId(evt) {
    if (!evt) return "";
    if (evt.target && evt.target.id) return evt.target.id;
    if (evt.target && evt.target.data && evt.target.data.id) return evt.target.data.id;
    if (evt.target && evt.target.getData) {
      try { return (evt.target.getData() || {}).id || ""; } catch (_) {}
    }
    return evt.item && evt.item.getID ? evt.item.getID() : "";
  }
  function render(options) {
    var lib = G6();
    var Graph = lib && lib.Graph;
    var host = options && options.host;
    if (!Graph || !host) return false;
    clear();
    runtime.lastHost = host;
    var layout = options.layout || "galaxy";
    var colors = colorSet(layout);
    var width = Math.max(960, host.clientWidth || 1040);
    var height = Math.max(620, host.clientHeight || 640);
    var sourceNodes = uniqueNodes(rankNodes(options.nodes || [], options.edges || [], options.focusNodeId ? 220 : 180, options.focusNodeId));
    var visible = new Set(sourceNodes.map(function (n) { return n.id; }));
    var sourceEdges = (options.edges || []).filter(function (e) { return visible.has(e.source) && visible.has(e.target); }).slice(0, options.focusNodeId ? 420 : 320);
    var degree = degreeMap(sourceEdges);
    var pos = computePositions(sourceNodes, sourceEdges, layout, width, height);
    host.innerHTML = "";
    host.className = "graph-visual graph-g6-host graph-layout-" + layout;
    host.style.background = colors.bg;
    var nodes = sourceNodes.map(function (n) {
      var p = pos[n.id] || {x: width / 2, y: height / 2};
      var size = sizeFor(n, degree, layout);
      var showLabel = n.kind === "package" || n.kind === "file" || size >= 22 || n.id === options.selectedNodeId || n.id === options.focusNodeId;
      return {
        id: n.id,
        data: {raw: n, degree: degree[n.id] || 0, label: n.label || n.id, kind: n.kind},
        style: {
          x: p.x,
          y: p.y,
          size: size,
          r: size / 2,
          fill: nodeColor(n, colors),
          stroke: layout === "blueprint" ? "rgba(219,234,254,0.85)" : "rgba(255,255,255,0.95)",
          lineWidth: n.id === options.selectedNodeId ? 4 : 2,
          labelText: showLabel ? labelText(n) : "",
          labelFill: colors.text,
          labelFontSize: n.kind === "package" ? 18 : size >= 24 ? 14 : 11,
          labelFontWeight: n.kind === "package" ? 700 : 500,
          labelBackground: true,
          labelBackgroundFill: layout === "orbit" ? "rgba(255,255,255,0.76)" : "rgba(2,6,23,0.34)",
          labelBackgroundRadius: 4,
          labelPadding: [2, 4]
        }
      };
    });
    var usedEdgeIds = new Set();
    var edges = sourceEdges.map(function (e, i) {
      var baseId = e.id || ("edge-" + e.source + "-" + e.target);
      var edgeId = baseId;
      if (usedEdgeIds.has(edgeId)) edgeId = baseId + "#" + i;
      usedEdgeIds.add(edgeId);
      return {
        id: edgeId,
        source: e.source,
        target: e.target,
        data: {raw: e, kind: e.kind || "related"},
        type: layout === "flow" || layout === "blueprint" ? "cubic-horizontal" : "line",
        style: {
          stroke: edgeColor(e, colors),
          lineWidth: e.kind === "calls" ? 1.8 : 1.05,
          opacity: e.kind === "calls" ? 0.62 : 0.30
        }
      };
    });
    runtime.nodeIds = nodes.map(function (n) { return n.id; });
    runtime.edgeIds = edges.map(function (e) { return e.id; });
    try {
      var graph = new Graph({
        container: host,
        width: width,
        height: height,
        autoFit: "view",
        autoResize: true,
        zoomRange: [0.15, 5],
        animation: {duration: 360, easing: "ease-out"},
        data: {nodes: nodes, edges: edges},
        node: {
          type: "circle",
          state: {
            selected: {lineWidth: 4, stroke: "#ffffff", shadowColor: "#60a5fa", shadowBlur: 22},
            active: {lineWidth: 3, stroke: "#fef08a", shadowColor: "#facc15", shadowBlur: 18},
            inactive: {opacity: 0.18}
          }
        },
        edge: {
          state: {
            active: {lineWidth: 2.4, opacity: 0.82},
            inactive: {opacity: 0.08}
          }
        },
        behaviors: [
          "drag-canvas",
          "drag-element",
          {type: "click-select", degree: Number(options.depth || 1), state: "active", neighborState: "selected", unselectedState: "inactive"}
        ].concat(options.zoomLocked ? [] : ["zoom-canvas"]),
        transforms: ["process-parallel-edges"]
      });
      graph.on("node:click", function (evt) {
        var id = eventNodeId(evt);
        if (id && options.onSelect) options.onSelect(id);
      });
      graph.on("node:dblclick", function (evt) {
        var id = eventNodeId(evt);
        if (id && options.onFocus) options.onFocus(id);
      });
      graph.on("node:pointerenter", function (evt) {
        var id = eventNodeId(evt);
        if (id && graph.setElementState) {
          try { graph.setElementState(id, ["active"]); } catch (_) {}
        }
      });
      graph.on("canvas:click", function () {
        if (graph.setElementState) {
          var states = {};
          runtime.nodeIds.forEach(function (id) { states[id] = []; });
          runtime.edgeIds.forEach(function (id) { states[id] = []; });
          try { graph.setElementState(states); } catch (_) {}
        }
      });
      runtime.graph = graph;
      var renderResult = graph.render();
      if (renderResult && typeof renderResult.then === "function") {
        renderResult.then(function () { runtime.lastRendered = true; }).catch(function () { runtime.lastRendered = false; });
      } else {
        runtime.lastRendered = true;
      }
      var badge = document.createElement("div");
      badge.className = "graph-engine-badge";
      badge.innerHTML = "<strong>G6</strong> · " + esc(sourceNodes.length) + " nodes / " + esc(sourceEdges.length) + " edges";
      host.appendChild(badge);
      return true;
    } catch (err) {
      clear();
      host.dataset.g6Error = err && err.message ? err.message : "G6 render failed";
      return false;
    }
  }
  function select(nodeId) {
    if (!runtime.graph || !runtime.graph.setElementState) return;
    var states = {};
    runtime.nodeIds.forEach(function (id) { states[id] = id === nodeId ? ["selected"] : []; });
    try { runtime.graph.setElementState(states); } catch (_) {}
  }
  window.CodeLatticeG6Graph = {
    version: "g6-5.1.1",
    available: function () { return !!(G6() && G6().Graph); },
    render: render,
    select: select,
    destroy: clear,
    lastRendered: function () { return runtime.lastRendered; }
  };
})();
