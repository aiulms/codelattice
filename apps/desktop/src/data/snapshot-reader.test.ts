// data/snapshot-reader + evidence-client 契约测试（G1 gate / 验收 1, 5, 7, 8）。
import { describe, it, expect, beforeAll } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildIndex,
  defaultRelationKey,
  directNeighbors,
  aggregateSelectionFacts,
  attachSelectionToChatMessage,
  graphViewForLevel,
  projectCoverageContext,
  relationKeyOf,
  staticLimitations,
} from "./snapshot-reader";
import { EvidenceClient } from "./evidence-client";
import { FakeDesktopTransport } from "../transport/fake-transport";
import type { SnapshotData } from "../types";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE = path.resolve(__dirname, "../../../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");

let data: SnapshotData;
let json: string;

beforeAll(() => {
  json = readFileSync(FIXTURE, "utf8");
  data = JSON.parse(json) as SnapshotData;
});

describe("relationKey identity (§6.1)", () => {
  it("is deterministic per (source, kind, target)", () => {
    const a = defaultRelationKey("n:a", "calls", "n:b");
    const b = defaultRelationKey("n:a", "calls", "n:b");
    expect(a).toBe(b);
    const c = defaultRelationKey("n:a", "imports", "n:b");
    expect(a).not.toBe(c);
    expect(a.startsWith("rel:sha256:")).toBe(true);
  });

  it("prefers the generator-provided relationKey when present", () => {
    expect(relationKeyOf({ source: "s", target: "t", kind: "calls", relationKey: "rel:x" })).toBe("rel:x");
  });
});

describe("buildIndex / directNeighbors (验收 1：先显示 snapshot 内直接关系)", () => {
  it("uses the stable library snapshot id when the caller provides it", () => {
    expect(buildIndex(data, "rust-portable-smoke.snapshot").snapshotId)
      .toBe("rust-portable-smoke.snapshot");
  });

  it("indexes nodes/edges and exposes direct upstream/downstream", () => {
    const idx = buildIndex(data);
    expect(idx.nodeById.size).toBe(data.graph.nodes.length);
    expect(idx.relationByKey.size).toBe(data.graph.edges.length);
    const someNode = data.graph.nodes[0];
    const { upstream, downstream } = directNeighbors(idx, someNode.id, "both");
    const all = [...upstream, ...downstream];
    // 所有边端点必须存在于节点集合（无 dangling）
    for (const r of all) {
      expect(idx.nodeById.has(r.sourceId)).toBe(true);
      expect(idx.nodeById.has(r.targetId)).toBe(true);
    }
  });

  it("filters direction", () => {
    const idx = buildIndex(data);
    const node = data.graph.nodes.find((n) =>
      [...idx.outEdges.get(n.id) ?? [], ...idx.inEdges.get(n.id) ?? []].length > 0);
    if (!node) return;
    const up = directNeighbors(idx, node.id, "upstream");
    expect(up.downstream).toEqual([]);
    const down = directNeighbors(idx, node.id, "downstream");
    expect(down.upstream).toEqual([]);
  });
});

describe("coverageContext (验收 8：不伪造分母)", () => {
  it("reports project scope only and marks knownIncomplete", () => {
    const cc = projectCoverageContext(data);
    expect(cc.scope).toBe("project");
    expect(cc.knownIncomplete).toBe(true);
    expect(cc.caveatRef).toBe("coverage:project:calls");
    expect(typeof cc.resolvedCalls).toBe("number");
    expect(cc.totalCalls).toBeGreaterThanOrEqual(cc.resolvedCalls);
  });
});

describe("graphViewForLevel", () => {
  it("symbol level returns the snapshot graph unchanged", () => {
    const view = graphViewForLevel(data, "symbol");
    expect(view.level).toBe("symbol");
    expect(view.nodes).toEqual(data.graph.nodes);
    expect(view.edges).toEqual(data.graph.edges);
  });

  it("module level maps moduleGraph and preserves count-sum invariant", () => {
    const view = graphViewForLevel(data, "module");
    expect(view.level).toBe("module");
    expect(view.nodes.length).toBe(data.moduleGraph?.modules.length);
    const countSum = view.edges.reduce((n, e) => n + (e.count ?? 1), 0);
    const expected = data.moduleGraph?.edges.reduce((n, e) => n + e.count, 0) ?? 0;
    expect(countSum).toBe(expected);
    for (const e of view.edges) {
      expect(view.nodes.some((n) => n.id === e.source)).toBe(true);
      expect(view.nodes.some((n) => n.id === e.target)).toBe(true);
      expect(e.source).not.toBe(e.target);
    }
  });

  it("file level only aggregates existing inter-file edges", () => {
    const view = graphViewForLevel(data, "file");
    expect(view.level).toBe("file");
    const fileOf = new Map(
      data.graph.nodes.map((n) => [n.id, n.kind === "file" ? (n.file || n.label) : (n.file || "")]),
    );
    let aggregatable = 0;
    for (const e of data.graph.edges) {
      const a = fileOf.get(e.source) || "";
      const b = fileOf.get(e.target) || "";
      if (a && b && a !== b) aggregatable += 1;
    }
    const countSum = view.edges.reduce((n, e) => n + (e.count ?? 1), 0);
    expect(countSum).toBe(aggregatable);
  });

  it("falls back to symbol view when moduleGraph is absent", () => {
    const { moduleGraph: _drop, ...rest } = data;
    const view = graphViewForLevel(rest, "module");
    expect(view.level).toBe("symbol");
    expect(view.nodes).toEqual(data.graph.nodes);
  });
});

describe("aggregateSelectionFacts", () => {
  it("returns null at symbol level", () => {
    expect(aggregateSelectionFacts(data, "symbol", {
      type: "node",
      nodeId: data.graph.nodes[0].id,
      snapshotId: "snap",
    })).toBeNull();
  });

  it("repeats moduleGraph counts for a module node", () => {
    const view = graphViewForLevel(data, "module");
    const node = view.nodes[0];
    const facts = aggregateSelectionFacts(data, "module", {
      type: "node",
      nodeId: node.id,
      snapshotId: "snap",
    });
    expect(facts?.kicker).toBe("模块");
    expect(facts?.title).toBe(node.label);
    expect(facts?.rows.some(([k]) => k === "文件数" || k === "出边（底层）")).toBe(true);
  });

  it("repeats count and minConfidence for an aggregated edge", () => {
    const view = graphViewForLevel(data, "module");
    const edge = view.edges[0];
    if (!edge) return;
    const facts = aggregateSelectionFacts(data, "module", {
      type: "relation",
      relationKey: relationKeyOf(edge),
      snapshotId: "snap",
    });
    expect(facts?.kicker).toBe("聚合边");
    expect(facts?.rows).toContainEqual(["底层边数", String(edge.count ?? 1)]);
  });

  it("lists each chosen module edge in a multi selection", () => {
    const mini: SnapshotData = {
      ...data,
      moduleGraph: {
        modules: [
          { id: "scripts", files: 1, symbols: 1 },
          { id: "lib", files: 1, symbols: 1 },
        ],
        edges: [
          { source: "scripts", target: "lib", count: 6, kinds: ["calls"], minConfidence: 0.55, reasons: ["external-command-invocation"] },
          { source: "lib", target: "scripts", count: 1, kinds: ["calls"], minConfidence: 0.8 },
        ],
        truncated: false,
      },
    };
    const view = graphViewForLevel(mini, "module");
    const facts = aggregateSelectionFacts(mini, "module", {
      type: "multi",
      snapshotId: "snap",
      nodeIds: [],
      relationKeys: view.edges.map((e) => relationKeyOf(e)),
    });
    expect(facts?.kicker).toBe("多选");
    expect(facts?.title).toBe("2 项");
    expect(facts?.rows[0][0]).toBe("边 1");
    expect(facts?.rows[1][0]).toBe("边 2");
    expect(facts?.rows[0][1]).toContain("scripts → lib");
  });
});

describe("attachSelectionToChatMessage", () => {
  it("appends the current graph selection so the model can resolve 这根线", () => {
    const out = attachSelectionToChatMessage(
      "你能看到这根线的相关关联点吗？",
      "聚合边：scripts → (unknown)\n底层边数：6",
    );
    expect(out).toContain("[当前图谱选择]");
    expect(out).toContain("scripts → (unknown)");
    expect(out).toContain("不要再索要 relationKey");
  });

  it("does not duplicate facts already present in an explain prompt", () => {
    const prompt = "请解释这条聚合边关系（已有边向上归并，不是新推断的依赖）：scripts → (unknown)";
    expect(attachSelectionToChatMessage(prompt, "聚合边：scripts → (unknown)")).toBe(prompt);
  });
});

describe("staticLimitations", () => {
  it("normalizes the dict shape emitted by the generator", () => {
    const list = staticLimitations(data);
    expect(list.length).toBeGreaterThan(0);
    expect(list[0]).toHaveProperty("id");
    expect(list[0]).toHaveProperty("text");
  });
});

describe("EvidenceClient (验收 1/5/7：preview 与 full 的区分)", () => {
  const meta = { id: "snap:test", rootLabel: "rust-portable-smoke", language: "rust", createdAt: "2026-08-05T00:00:00Z" };

  it("preview answers directly from the snapshot without transport", () => {
    const transport = new FakeDesktopTransport(json, meta, { failEvidenceQueries: true });
    const idx = buildIndex(data);
    const client = new EvidenceClient(transport, idx, data);
    const node = data.graph.nodes.find((n) => n.kind === "symbol");
    if (!node) return;
    const ctx = client.getNodeContextPreview(node.id);
    expect(ctx.origin).toBe("preview");
    expect(Array.isArray(ctx.directCallers)).toBe(true);
    expect(ctx.coverageContext.scope).toBe("project");
  });

  it("full query degrades to preview when the query service is unavailable (验收 7)", async () => {
    const transport = new FakeDesktopTransport(json, meta, { failEvidenceQueries: true });
    const idx = buildIndex(data);
    const client = new EvidenceClient(transport, idx, data);
    const edge = data.graph.edges.find((e) => e.kind === "calls");
    if (!edge) return;
    const bundle = await client.getEdgeEvidenceFull("snap:test", relationKeyOf(edge));
    expect(bundle.origin).toBe("preview");
    expect(bundle.selection.relationKey).toBe(relationKeyOf(edge));
  });

  it("selection itself performs zero network calls (验收 2)", () => {
    const idx = buildIndex(data);
    const selector = { type: "node", nodeId: data.graph.nodes[0].id, snapshotId: "snap:test" };
    // 一个任何调用都会抛错的 transport：预览路径不得触碰它
    const hostile = new Proxy({} as FakeDesktopTransport, {
      get: () => () => { throw new Error("must not be called"); },
    });
    const client = new EvidenceClient(hostile, idx, data);
    expect(() => client.getNodeContextPreview(selector.nodeId)).not.toThrow();
  });
});
