// GraphPane — 图谱画布面板：挂载 G6Adapter + GraphController。
// 选择通过 GraphSelectionStore 驱动高亮；不直接持有模型/网络逻辑。
import { useEffect, useRef } from "react";
import type { GraphSelection, SnapshotData, DesktopTransport } from "../types";
import { G6GraphAdapter } from "../graph/g6-adapter";
import type { G6Like } from "../graph/g6-adapter";
import { GraphController } from "../graph/graph-controller";
import type { GraphSelectionStore } from "../state/graph-selection";

export function GraphPane(props: {
  selection: GraphSelection;
  transport: DesktopTransport;
  snapshot?: SnapshotData | null;
  store: GraphSelectionStore;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const adapterRef = useRef<G6GraphAdapter | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const hostEl: HTMLElement = host;
    let adapter: G6GraphAdapter | null = null;
    let disposed = false;

    async function mount() {
      const snap = props.snapshot ?? (await loadFirstSnapshot(props.transport));
      if (disposed || !snap) return;
      const { buildIndex } = await import("../data/snapshot-reader");
      const index = buildIndex(snap);
      const controller = new GraphController(props.store);
      // 动态加载 G6（WKWebView 首屏后可异步；失败不阻塞事实面板）
      const g6mod = await import("@antv/g6");
      const factory = g6mod.Graph as unknown as G6Like;
      adapter = new G6GraphAdapter(controller, factory, hostEl);
      const ok = adapter.render(index.nodes, index.edges, { selection: props.selection });
      adapterRef.current = adapter;
      if (!ok) hostEl.dataset.g6Error = "render-failed";
    }
    void mount();

    return () => {
      disposed = true;
      adapterRef.current?.destroy();
      adapterRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.store, props.transport]);

  useEffect(() => {
    adapterRef.current?.setSelection(props.selection);
  }, [props.selection]);

  return (
    <section className="panel graph" data-testid="graph-pane">
      <div
        ref={hostRef}
        className="graph-host"
        data-testid="graph-host"
        style={{ width: "100%", height: "100%" }}
      />
    </section>
  );
}

async function loadFirstSnapshot(transport: DesktopTransport): Promise<SnapshotData | null> {
  try {
    const snaps = await transport.listSnapshots();
    if (snaps.length === 0) return null;
    return transport.loadSnapshot(snaps[0].id);
  } catch {
    return null;
  }
}
