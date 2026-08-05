// Selftest harness — 在真实 WKWebView 内驱动 G6（P0-F1 #7 / G1 gate）。
//
// 触发方式：`CODELATTICE_SELFTEST=1 tauri dev`（Rust 侧命令返回 true）。
// 执行：真实 bounded snapshot → G6 mount → node click → edge click →
// resize → 重复 mount/unmount ×3 → 结果经 workbench_smoke_report 写盘。
import type { DesktopTransport } from "../types";
import { buildIndex, relationKeyOf } from "../data/snapshot-reader";
import type { SnapshotIndex } from "../data/snapshot-reader";
import { G6GraphAdapter } from "../graph/g6-adapter";
import type { G6GraphLike } from "../graph/g6-adapter";
import { GraphSelectionStore } from "../state/graph-selection";
import { ConversationStore } from "../state/conversation";

type ConstructorOfG6 = new (config: unknown) => G6GraphLike;

type SelftestStep = { name: string; pass: boolean; detail?: string };

export async function maybeRunSelftest(transport: DesktopTransport): Promise<void> {
  let enabled = false;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    enabled = await invoke<boolean>("workbench_selftest_enabled");
  } catch {
    enabled = false;
  }
  if (!enabled) return;

  // I-fix: 端口检测 — 等待 Vite dev server 就绪再执行 selftest
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    let portReady = false;
    for (let i = 0; i < 10; i++) {
      const result = await invoke<{ port: number; reachable: boolean }>("workbench_check_port", { port: 1420 });
      if (result.reachable) { portReady = true; break; }
      await new Promise((r) => setTimeout(r, 500));
    }
    if (!portReady) {
      await transport.writeSmokeReport({
        schemaVersion: "codelattice.selftest.v1",
        finishedAt: new Date().toISOString(),
        fatal: "Vite dev server port 1420 not reachable after 5s",
        steps: [],
        allPass: false,
      });
      return;
    }
  } catch {
    // 端口检测不可用时继续执行（非阻断）
  }

  const steps: SelftestStep[] = [];
  const step = (name: string, fn: () => void | Promise<void>) =>
    Promise.resolve()
      .then(fn)
      .then(() => steps.push({ name, pass: true }))
      .catch((e) => steps.push({ name, pass: false, detail: String(e?.message ?? e) }));

  let adapter: G6GraphAdapter | null = null;
  let graphRef: G6GraphLike | null = null;
  let index: SnapshotIndex | null = null;
  let host: HTMLDivElement | null = null;
  const selStore = new GraphSelectionStore();
  const convStore = new ConversationStore("");

  async function loadIndex(): Promise<SnapshotIndex> {
    const snaps = await transport.listSnapshots();
    if (snaps.length === 0) throw new Error("no snapshots");
    const data = await transport.loadSnapshot(snaps[0].id);
    const idx = buildIndex(data);
    if (idx.nodes.length === 0 || idx.edges.length === 0) throw new Error("empty snapshot graph");
    return idx;
  }

  async function mountAdapter(): Promise<void> {
    if (!host) throw new Error("host missing");
    const g6mod = await import("@antv/g6");
    const factory = g6mod.Graph as unknown as ConstructorOfG6;
    adapter = new G6GraphAdapter(
      {
        onSelectNode: (nodeId) => {
          if (index) selStore.dispatch({ type: "select-node", nodeId, snapshotId: index.snapshotId });
        },
        onFocusNode: (nodeId) => {
          if (index) selStore.dispatch({ type: "select-node", nodeId, snapshotId: index.snapshotId });
        },
        onHoverNode: () => {},
        onSelectEdge: (relationKey) => {
          if (index) selStore.dispatch({ type: "select-relation", relationKey, snapshotId: index.snapshotId });
        },
        onHoverEdge: () => {},
        onCanvasClick: () => selStore.dispatch({ type: "clear" }),
      },
      factory,
      host,
    );
    if (!index) throw new Error("no index");
    if (!adapter.render(index.nodes, index.edges, {})) {
      // 诊断：把 adapter 捕获的真实错误暴露到报告，避免“render returned false”
      // 无从排查（G1 复现时发现 detail 缺失）。
      const why = adapter.lastError ? `: ${adapter.lastError}` : "";
      throw new Error(`g6 render returned false${why}`);
    }
    graphRef = adapter.getGraph();
  }

  try {
    await step("load snapshot via transport", async () => {
      index = await loadIndex();
    });

    await step("mount G6 with real snapshot data", async () => {
      host = document.createElement("div");
      host.style.cssText = "position:fixed;left:-10000px;top:0;width:1200px;height:800px;";
      document.body.appendChild(host);
      await mountAdapter();
      if (!graphRef) throw new Error("graph not mounted");
    });

    await step("node click updates GraphSelectionStore", () => {
      if (!graphRef || !graphRef.emit || !index) throw new Error("graph not mounted");
      const nodeId = index.nodes[0].id;
      graphRef.emit("node:click", { target: { id: nodeId } });
      const s = selStore.getState();
      if (s.type !== "node" || s.nodeId !== nodeId) throw new Error("selection not updated");
    });

    await step("edge click updates GraphSelectionStore (relationKey identity)", () => {
      if (!graphRef || !graphRef.emit || !index || !adapter) throw new Error("graph not mounted");
      const edge = index.edges[0];
      // 与 adapter 一致：语义身份 = relationKeyOf（无 relationKey 时默认 sha256 key）
      const key = relationKeyOf(edge);
      // 元素 id 是唯一实例身份（平行边各自独立）；事件回调必须映射回 relationKey
      const elemIds = adapter.edgeElementIds().get(key);
      if (!elemIds || elemIds.length === 0) throw new Error("edge element not rendered");
      graphRef.emit("edge:click", { target: { id: elemIds[0] } });
      const s = selStore.getState();
      if (s.type !== "relation" || s.relationKey !== key) throw new Error("edge selection not updated");
    });

    await step("window resize does not throw", () => {
      window.dispatchEvent(new Event("resize"));
      selStore.dispatch({ type: "clear" });
    });

    await step("repeat mount/unmount ×3", async () => {
      for (let i = 0; i < 3; i++) {
        adapter?.destroy();
        adapter = null;
        graphRef = null;
        await mountAdapter();
      }
    });

    await step("20 selection rounds (node/relation/clear) keep store consistent", async () => {
      if (!index) throw new Error("no index");
      for (let i = 0; i < 20; i++) {
        if (i % 3 === 0) {
          const nodeId = index.nodes[i % index.nodes.length].id;
          selStore.dispatch({ type: "select-node", nodeId, snapshotId: index.snapshotId });
          if (selStore.getState().type !== "node") throw new Error(`round ${i}: node selection lost`);
        } else if (i % 3 === 1) {
          const edge = index.edges[i % index.edges.length];
          const key = relationKeyOf(edge);
          selStore.dispatch({ type: "select-relation", relationKey: key, snapshotId: index.snapshotId });
          if (selStore.getState().type !== "relation") throw new Error(`round ${i}: relation selection lost`);
        } else {
          selStore.dispatch({ type: "clear" });
          if (selStore.getState().type !== "none") throw new Error(`round ${i}: clear failed`);
        }
      }
    });

    await step("ConversationStore remains independent of graph selection", () => {
      convStore.dispatch({ type: "pin", scopeType: "node", id: "n:pinned", snapshotId: "snap:1" });
      if (convStore.getState().pinnedScope?.id !== "n:pinned") throw new Error("pin lost");
    });

    const report = {
      schemaVersion: "codelattice.selftest.v1",
      finishedAt: new Date().toISOString(),
      env: { userAgent: navigator.userAgent, webview: "wkwebview" },
      steps,
      allPass: steps.every((s) => s.pass),
    };
    await transport.writeSmokeReport(report as unknown as Record<string, unknown>);
  } catch (e) {
    await transport.writeSmokeReport({
      schemaVersion: "codelattice.selftest.v1",
      finishedAt: new Date().toISOString(),
      fatal: String(e),
      steps,
      allPass: false,
    });
  }
}
