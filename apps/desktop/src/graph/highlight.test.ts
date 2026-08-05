// highlight 语义测试（F0 字符化基线 → 新 Workbench 行为等价，验收 1/2）。
import { describe, it, expect } from "vitest";
import { computeHighlight } from "./highlight";
import type { GraphSelection, SnapshotEdge, SnapshotNode } from "../types";
import { defaultRelationKey } from "../data/snapshot-reader";

const nodes: SnapshotNode[] = [
  { id: "n:a", label: "a", kind: "symbol" },
  { id: "n:b", label: "b", kind: "symbol" },
  { id: "n:c", label: "c", kind: "symbol" },
  { id: "n:f1", label: "f1.rs", kind: "file" },
  { id: "n:pkg", label: "pkg", kind: "package" },
];
const edges: SnapshotEdge[] = [
  { source: "n:a", target: "n:b", kind: "calls" },
  { source: "n:b", target: "n:c", kind: "calls" },
  { source: "n:f1", target: "n:a", kind: "defines" },
];
const rk = (s: string, t: string) => defaultRelationKey(s, "calls", t);

describe("computeHighlight (F0 字符化冻结语义)", () => {
  it("no selection → nothing highlighted, nothing dimmed", () => {
    const hl = computeHighlight(nodes, edges, { type: "none" });
    expect(hl.selected.size).toBe(0);
    expect(hl.dimmed.size).toBe(0);
  });

  it("node selection → node + incident edges selected, neighbors marked, others dimmed", () => {
    const hl = computeHighlight(nodes, edges, { type: "node", nodeId: "n:b", snapshotId: "s" });
    expect(hl.selected.has("n:b")).toBe(true);
    expect(hl.selected.has(rk("n:a", "n:b"))).toBe(true);
    expect(hl.selected.has(rk("n:b", "n:c"))).toBe(true);
    expect(hl.neighbor.has("n:a")).toBe(true);
    expect(hl.neighbor.has("n:c")).toBe(true);
    // 非邻接：f1 与 pkg dimmed（与旧 adapter 字符化一致）
    expect(hl.dimmed.has("n:f1")).toBe(true);
    expect(hl.dimmed.has("n:pkg")).toBe(true);
    expect(hl.dimmed.has("n:a")).toBe(false);
  });

  it("relation selection → edge + both endpoints highlighted", () => {
    const key = rk("n:a", "n:b");
    const hl = computeHighlight(nodes, edges, { type: "relation", relationKey: key, snapshotId: "s" });
    expect(hl.selected.has(key)).toBe(true);
    expect(hl.neighbor.has("n:a")).toBe(true);
    expect(hl.neighbor.has("n:b")).toBe(true);
    expect(hl.dimmed.has("n:c")).toBe(true);
  });

  it("unknown node/relation → empty highlight, never crashes", () => {
    const hl1 = computeHighlight(nodes, edges, { type: "node", nodeId: "n:ghost", snapshotId: "s" });
    expect(hl1.selected.size).toBe(0);
    const hl2 = computeHighlight(nodes, edges, { type: "relation", relationKey: "rel:ghost", snapshotId: "s" });
    expect(hl2.selected.size).toBe(0);
  });

  it("is pure: same input → same output", () => {
    const sel: GraphSelection = { type: "node", nodeId: "n:b", snapshotId: "s" };
    const a = computeHighlight(nodes, edges, sel);
    const b = computeHighlight(nodes, edges, sel);
    expect([...a.selected].sort()).toEqual([...b.selected].sort());
  });
});
