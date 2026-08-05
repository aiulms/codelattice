// data/snapshot-reader + evidence-client 契约测试（G1 gate / 验收 1, 5, 7, 8）。
import { describe, it, expect, beforeAll } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildIndex,
  defaultRelationKey,
  directNeighbors,
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
