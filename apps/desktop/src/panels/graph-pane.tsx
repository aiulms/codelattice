// GraphPane — 图谱画布：G6Adapter + 模块/文件/符号层级。
// 选择通过 GraphSelectionStore 驱动高亮；悬停只改状态，不重跑布局。
import { useEffect, useMemo, useRef, useState } from "react";
import type { GraphLevel, GraphSelection, SnapshotData, DesktopTransport } from "../types";
import { G6GraphAdapter } from "../graph/g6-adapter";
import type { G6Like } from "../graph/g6-adapter";
import { GraphController } from "../graph/graph-controller";
import type { GraphSelectionStore } from "../state/graph-selection";
import { graphViewForLevel } from "../data/snapshot-reader";

export function GraphPane(props: {
  selection: GraphSelection;
  transport: DesktopTransport;
  snapshot?: SnapshotData | null;
  snapshotId?: string;
  store: GraphSelectionStore;
  level: GraphLevel;
  onLevelChange(level: GraphLevel): void;
  hasModuleGraph: boolean;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const adapterRef = useRef<G6GraphAdapter | null>(null);
  const [multiSelect, setMultiSelect] = useState(false);
  const multiRef = useRef(false);
  multiRef.current = multiSelect;
  const view = useMemo(
    () => (props.snapshot ? graphViewForLevel(props.snapshot, props.level) : null),
    [props.snapshot, props.level],
  );

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !view) return;
    const hostEl: HTMLElement = host;
    const mountedView = view;
    let adapter: G6GraphAdapter | null = null;
    let disposed = false;

    async function mount() {
      const { buildIndex } = await import("../data/snapshot-reader");
      const index = props.snapshot
        ? buildIndex(props.snapshot, props.snapshotId ?? props.snapshot.generatedAt)
        : null;
      if (disposed || !index) return;
      const controller = new GraphController(props.store, index.snapshotId, () => multiRef.current);
      const g6mod = await import("@antv/g6");
      const factory = g6mod.Graph as unknown as G6Like;
      adapter = new G6GraphAdapter({
        onSelectNode: (id, add) => controller.onSelectNode(id, add),
        onFocusNode: (id) => controller.onFocusNode(id),
        onHoverNode: (id) => adapterRef.current?.setHover(id ? { type: "node", id } : null),
        onSelectEdge: (key, add) => controller.onSelectEdge(key, add),
        onHoverEdge: (key) => adapterRef.current?.setHover(key ? { type: "edge", id: key } : null),
        onCanvasClick: () => controller.onCanvasClick(),
      }, factory, hostEl);
      const ok = adapter.render(mountedView.nodes, mountedView.edges, { selection: props.selection });
      adapterRef.current = adapter;
      if (!ok) hostEl.dataset.g6Error = "render-failed";
    }
    void mount();

    return () => {
      disposed = true;
      adapterRef.current?.destroy();
      adapterRef.current = null;
    };
    // 层级或 snapshot 变化才重挂；选择变化走 setSelection，避免力导向重来一遍
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.store, props.transport, props.snapshotId, props.level, view]);

  useEffect(() => {
    adapterRef.current?.setSelection(props.selection);
  }, [props.selection]);

  const levels: Array<{ id: GraphLevel; label: string; disabled?: boolean }> = [
    { id: "module", label: "模块", disabled: !props.hasModuleGraph },
    { id: "file", label: "文件" },
    { id: "symbol", label: "符号" },
  ];

  return (
    <section className="panel graph" data-testid="graph-pane">
      <div
        className="graph-toolbar"
        data-testid="graph-level-switch"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
      >
        {levels.map((item) => (
          <button
            key={item.id}
            type="button"
            className={props.level === item.id ? "active" : ""}
            disabled={item.disabled}
            data-testid={`graph-level-${item.id}`}
            onClick={() => props.onLevelChange(item.id)}
          >
            {item.label}
          </button>
        ))}
        <span className="graph-toolbar-sep" aria-hidden="true" />
        <button
          type="button"
          className={multiSelect ? "active" : ""}
          data-testid="graph-multi-select"
          aria-pressed={multiSelect}
          title="连续点选多条边或节点；按住 Shift 或 ⌘ 点击同样有效"
          onClick={() => setMultiSelect((v) => !v)}
        >
          多选
        </button>
      </div>
      <div
        ref={hostRef}
        className="graph-host"
        data-testid="graph-host"
        style={{ width: "100%", height: "100%" }}
      />
    </section>
  );
}
