// E2E: Inspector race condition — 验证 preview → full async 加载不产生竞态。
//
// 模拟用户快速切换 node 选择时，旧的 full 加载不应覆盖新的 preview。
import { describe, it, expect } from "vitest";
import { EvidenceClient } from "../data/evidence-client";
import { buildIndex } from "../data/snapshot-reader";
import { FakeDesktopTransport } from "../transport/fake-transport";
import type { SnapshotData } from "../types";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE = path.resolve(__dirname, "../../../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json");

describe("Inspector race condition E2E", () => {
  it("full evidence load does not override newer preview", async () => {
    const json = readFileSync(FIXTURE, "utf-8");
    const data = JSON.parse(json) as SnapshotData;
    const meta = { id: "snap:test", rootLabel: "test", language: "rust", createdAt: "2026-01-01T00:00:00Z" };
    const transport = new FakeDesktopTransport(json, meta);
    const index = buildIndex(data);
    const client = new EvidenceClient(transport, index, data);

    // 模拟快速切换：先选 node A，发起 full load，然后立即切换到 node B
    if (index.nodes.length < 2) return; // fixture 不足

    const nodeA = index.nodes[0].id;
    const nodeB = index.nodes[1].id;

    // Start full load for A (uses transport — may fall back to preview)
    const fullAPromise = client.getNodeContextFull(index.snapshotId, nodeA);

    // Immediately switch to B — preview should show immediately
    const previewB = client.getNodeContextPreview(nodeB);
    expect(previewB.nodeId).toBe(nodeB);
    expect(previewB.origin).toBe("preview");

    // Wait for A's full load to complete
    const fullA = await fullAPromise;

    // A's full data should have correct nodeId (not B's)
    // Note: transport may return its own nodeId or fall back to preview;
    // either way, it must not be B's data
    expect(fullA.nodeId).not.toBe(nodeB);
    // B's preview is still B's
    expect(previewB.nodeId).toBe(nodeB);
  });

  it("preview is available synchronously without transport call", () => {
    const json = readFileSync(FIXTURE, "utf-8");
    const data = JSON.parse(json) as SnapshotData;

    // Use a hostile transport that throws on any call
    const hostile = new Proxy({} as FakeDesktopTransport, {
      get() {
        throw new Error("transport should not be called for preview");
      },
    });

    const index = buildIndex(data);
    const client = new EvidenceClient(hostile, index, data);
    const node = index.nodes[0];

    // Preview must not touch transport
    const preview = client.getNodeContextPreview(node.id);
    expect(preview).toBeDefined();
    expect(preview.origin).toBe("preview");
  });

  it("edge evidence full falls back to preview when transport fails", async () => {
    const json = readFileSync(FIXTURE, "utf-8");
    const data = JSON.parse(json) as SnapshotData;
    const meta = { id: "snap:test", rootLabel: "test", language: "rust", createdAt: "2026-01-01T00:00:00Z" };
    const transport = new FakeDesktopTransport(json, meta, { failEvidenceQueries: true });
    const index = buildIndex(data);
    const client = new EvidenceClient(transport, index, data);

    const edge = index.edges[0];
    if (!edge) return;

    const key = index.relationByKey ? [...index.relationByKey.keys()][0] : "";
    if (!key) return;

    const full = await client.getEdgeEvidenceFull(index.snapshotId, key);
    // Should fall back to preview (not throw)
    expect(full).toBeDefined();
    expect(full.origin).toBe("preview");
  });
});
