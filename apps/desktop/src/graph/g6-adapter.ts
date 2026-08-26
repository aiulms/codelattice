// G6GraphAdapter — 新 Workbench 的 G6 渲染适配层（P0-A Track A #2 / F1 #7）。
//
// 与旧 graph-g6.js 不同：edge 元素携带 relationKey（语义身份，不是布局 id），
// 并发出 edge click / edge hover 事件（F0 字符化确认的 legacy gap）。
// G6 只做渲染/交互；语义、高亮、选择所有权在 GraphController。
import type { GraphSelection, SnapshotEdge, SnapshotNode } from "../types";
import { relationKeyOf } from "../data/snapshot-reader";
import { computeHighlight } from "./highlight";
import type { HighlightHover } from "./highlight";

export interface G6AdapterCallbacks {
  onSelectNode(nodeId: string, additive?: boolean): void;
  onFocusNode(nodeId: string): void;
  onHoverNode(nodeId: string | null): void;
  onSelectEdge(relationKey: string, additive?: boolean): void;
  onHoverEdge(relationKey: string | null): void;
  onCanvasClick(): void;
}

export interface G6Like {
  new (config: unknown): G6GraphLike;
}
export interface G6GraphLike {
  on(event: string, fn: (evt: { target?: { id?: string }; originalEvent?: unknown; targetType?: string }) => void): void;
  render(): Promise<void> | void;
  destroy(): void;
  setElementState(states: Record<string, string[]>): void;
  emit?: (event: string, evt: unknown) => void;
  resize?: (w: number, h: number) => void;
  focusElement?: (id: string | string[], options?: unknown) => Promise<void> | void;
}

export interface RenderOptions {
  selection?: GraphSelection;
  hover?: HighlightHover;
  focusNodeId?: string;
  onRenderError?(err: Error): void;
}

/**
 * G6 边元素 id —— 唯一实例身份，与语义身份（relationKey）分离（§6.1）。
 *
 * - occurrenceKey 存在时：直接用 occurrenceKey（事实层提供的稳定调用位置身份）；
 * - 否则 relation-level：`<relationKey>#<全局序号>`。序号仅保证 G6 元素 id 唯一，
 *   是布局实例身份，明确不做跨版本稳定承诺（与 §6.1「排序序号不能宣称稳定身份」一致）。
 */
export function edgeElementId(e: SnapshotEdge, index: number): string {
  if (e.occurrenceKey) return e.occurrenceKey;
  return `${relationKeyOf(e)}#${index}`;
}

/** 轻量依赖注入：测试传 fake，生产传 @antv/g6。 */
export class G6GraphAdapter {
  private graph: G6GraphLike | null = null;
  private lastNodes: SnapshotNode[] = [];
  private lastEdges: SnapshotEdge[] = [];
  private lastSelection: GraphSelection = { type: "none" };
  private lastHover: HighlightHover = null;
  private lastTopology = "";
  private edgeElementToRelation = new Map<string, string>();
  public lastError: string | null = null;

  constructor(
    private readonly callbacks: G6AdapterCallbacks,
    private readonly g6Factory: G6Like,
    private readonly container: HTMLElement,
  ) {}

  render(nodes: SnapshotNode[], edges: SnapshotEdge[], opts: RenderOptions = {}): boolean {
    this.lastNodes = nodes;
    this.lastEdges = edges;
    this.lastSelection = opts.selection ?? this.lastSelection;
    if (opts.hover !== undefined) this.lastHover = opts.hover;
    const topology = topologyKey(nodes, edges);
    if (this.graph && topology === this.lastTopology) {
      this.applyHighlight(this.graph);
      return true;
    }
    // destroy 会清空 lastTopology；新拓扑写入必须放在 destroy 之后，
    // 否则紧接着的 setSelection 会误判拓扑变化并重挂力导向。
    this.destroy();
    this.lastTopology = topology;
    const width = Math.max(600, this.container.clientWidth || 800);
    const height = Math.max(400, this.container.clientHeight || 600);
    const hl = computeHighlight(nodes, edges, this.lastSelection, this.lastHover);

    this.edgeElementToRelation.clear();
    const g6Edges = edges.map((e, i) => {
      const key = relationKeyOf(e);
      const elemId = edgeElementId(e, i); // 唯一元素 id；平行边各自独立渲染
      this.edgeElementToRelation.set(elemId, key);
      return {
        id: elemId,
        source: e.source,
        target: e.target,
        data: { raw: e, kind: e.kind, relationKey: key },
        style: edgeStyle(e, key, hl),
      };
    });

    const g6Nodes = nodes.map((n) => {
      const isSel = hl.selected.has(n.id);
      const isNb = hl.neighbor.has(n.id);
      return {
        id: n.id,
        data: { raw: n, kind: n.kind, selected: isSel, neighbor: isNb },
        style: {
          size: isSel ? 22 : isNb ? 16 : 12,
          fill: isSel ? "#f59e0b" : n.kind === "symbol" ? "#7dd3fc" : n.kind === "file" ? "#6366f1" : "#94a3b8",
          opacity: hl.dimmed.has(n.id) ? 0.18 : 0.96,
          // 语言身份角标：快照节点自述，缺席即未知（不渲染角标），前端不重算
          ...(n.language
            ? {
                badges: [
                  {
                    text: n.language,
                    placement: "right-top",
                    fontSize: 9,
                    padding: [1, 4, 1, 4],
                    backgroundFill: "#e8eef5",
                    fill: "#334e6b",
                  },
                ],
              }
            : {}),
          labelText: n.label.slice(0, 30),
          labelFill: "#172033",
          labelFontSize: 13,
          labelPlacement: "bottom",
          labelOffsetY: 7,
          labelBackground: true,
          labelBackgroundFill: "#ffffff",
          labelBackgroundOpacity: 0.88,
          labelBackgroundPadding: [2, 4, 2, 4],
        },
      };
    });

    try {
      const graph = new this.g6Factory({
        container: this.container,
        width,
        height,
        autoResize: true,
        autoFit: "view",
        padding: 40,
        zoomRange: [0.08, 4],
        // 源数据不携带布局坐标；显式 force + collision，避免所有节点落在原点。
        // nodeSize 同时覆盖标签附近的呼吸空间，不能靠隐藏标签掩盖重叠。
        layout: {
          type: "d3-force",
          preventOverlap: true,
          nodeSize: 58,
          nodeSpacing: 18,
          linkDistance: 120,
          nodeStrength: -260,
          collideStrength: 1,
          collideIterations: 4,
          iterations: 220,
          animation: false,
        },
        data: { nodes: g6Nodes, edges: g6Edges },
        node: {
          state: {
            selected: { size: 22, fill: "#f59e0b", opacity: 1 },
            neighbor: { size: 16, opacity: 0.96 },
            hovered: { size: 18, fill: "#fb923c", opacity: 1 },
            dimmed: { opacity: 0.16 },
          },
        },
        edge: {
          state: {
            selected: { lineWidth: 3.4, stroke: "#f59e0b", opacity: 0.96 },
            hovered: { lineWidth: 2.8, stroke: "#fb923c", opacity: 0.9 },
            dimmed: { opacity: 0.08 },
          },
        },
        behaviors: ["drag-canvas", "zoom-canvas", "drag-element"],
      });
      graph.on("node:click", (evt) => {
        const id = evt.target?.id;
        if (id) this.callbacks.onSelectNode(id, clickIsAdditive(evt));
      });
      graph.on("node:dblclick", (evt) => {
        const id = evt.target?.id;
        if (id) this.callbacks.onFocusNode(id);
      });
      graph.on("node:pointerenter", (evt) => {
        const id = evt.target?.id;
        if (id) this.callbacks.onHoverNode(id);
      });
      graph.on("node:pointerleave", () => this.callbacks.onHoverNode(null));
      graph.on("edge:click", (evt) => {
        const key = evt.target?.id ? this.edgeElementToRelation.get(evt.target.id) : undefined;
        if (key) this.callbacks.onSelectEdge(key, clickIsAdditive(evt));
      });
      graph.on("edge:pointerenter", (evt) => {
        const key = evt.target?.id ? this.edgeElementToRelation.get(evt.target.id) : undefined;
        if (key) this.callbacks.onHoverEdge(key);
      });
      graph.on("edge:pointerleave", () => this.callbacks.onHoverEdge(null));
      graph.on("canvas:click", (evt) => {
        const targetType = (evt as { targetType?: string }).targetType;
        // 部分 G6 版本点中边/节点仍会再发 canvas:click，不能把多选清掉
        if (targetType === "node" || targetType === "edge") return;
        this.callbacks.onCanvasClick();
      });
      const result = graph.render();
      if (result && typeof result.then === "function") {
        result.catch((err: unknown) => this.fail(err));
      }
      this.graph = graph;
      return true;
    } catch (err) {
      this.fail(err as Error);
      return false;
    }
  }

  private fail(err: unknown) {
    this.lastError = String((err as { message?: string })?.message ?? err);
    console.error("[g6-adapter] render failed", this.lastError);
  }

  setSelection(selection: GraphSelection): void {
    this.lastSelection = selection;
    this.render(this.lastNodes, this.lastEdges, { selection });
    this.frameSelection(selection);
  }

  setHover(hover: HighlightHover): void {
    this.lastHover = hover;
    if (this.graph) this.applyHighlight(this.graph);
  }

  private applyHighlight(graph: G6GraphLike): void {
    const hl = computeHighlight(this.lastNodes, this.lastEdges, this.lastSelection, this.lastHover);
    const states: Record<string, string[]> = {};
    for (const n of this.lastNodes) {
      if (hl.selected.has(n.id)) states[n.id] = ["selected"];
      else if (hl.hovered.has(n.id)) states[n.id] = ["hovered"];
      else if (hl.neighbor.has(n.id)) states[n.id] = ["neighbor"];
      else if (hl.dimmed.has(n.id)) states[n.id] = ["dimmed"];
      else states[n.id] = [];
    }
    for (const [elemId, key] of this.edgeElementToRelation) {
      if (hl.selected.has(key)) states[elemId] = ["selected"];
      else if (hl.hovered.has(key)) states[elemId] = ["hovered"];
      else if (hl.dimmed.has(key)) states[elemId] = ["dimmed"];
      else states[elemId] = [];
    }
    graph.setElementState(states);
  }

  /** 把镜头拉到选中的节点或边两端，模型看不到图，但人眼要对准当前线。 */
  private frameSelection(selection: GraphSelection): void {
    const graph = this.graph;
    if (!graph?.focusElement) return;
    const ids = frameIds(selection, this.lastEdges);
    if (ids.length === 0) return;
    try {
      void graph.focusElement(ids.length === 1 ? ids[0] : ids, { animation: true, easing: "ease-cubic" });
    } catch { /* G6 版本若无 focusElement 则保持当前视口 */ }
  }

  /** 仅供 selftest/测试注入事件用；业务代码不得依赖。 */
  getGraph(): G6GraphLike | null {
    return this.graph;
  }

  /** 已渲染边元素：relationKey → 元素 id 列表（selftest/测试注入事件用）。 */
  edgeElementIds(): Map<string, string[]> {
    const out = new Map<string, string[]>();
    for (const [elemId, key] of this.edgeElementToRelation) {
      const list = out.get(key) ?? [];
      list.push(elemId);
      out.set(key, list);
    }
    return out;
  }

  destroy(): void {
    if (this.graph) {
      try { this.graph.destroy(); } catch { /* already destroyed */ }
      this.graph = null;
    }
    this.lastTopology = "";
  }
}

function topologyKey(nodes: SnapshotNode[], edges: SnapshotEdge[]): string {
  return `${nodes.map((n) => n.id).join("\n")}\n${edges.map((e, i) => edgeElementId(e, i)).join("\n")}`;
}

function clickIsAdditive(evt: { originalEvent?: unknown }): boolean {
  const oe = evt.originalEvent as { shiftKey?: boolean; metaKey?: boolean } | undefined;
  return !!(oe && (oe.shiftKey || oe.metaKey));
}

function frameIds(selection: GraphSelection, edges: SnapshotEdge[]): string[] {
  if (selection.type === "node") return [selection.nodeId];
  if (selection.type === "relation") {
    const e = edges.find((x) => relationKeyOf(x) === selection.relationKey);
    return e ? [e.source, e.target] : [];
  }
  if (selection.type === "multi") {
    const ids = [...selection.nodeIds];
    for (const key of selection.relationKeys) {
      const e = edges.find((x) => relationKeyOf(x) === key);
      if (e) ids.push(e.source, e.target);
    }
    return [...new Set(ids)];
  }
  return [];
}

function isWeakEdge(e: SnapshotEdge): boolean {
  return typeof e.confidence === "number" && e.confidence < 0.7;
}

function edgeStyle(e: SnapshotEdge, key: string, hl: ReturnType<typeof computeHighlight>) {
  const selected = hl.selected.has(key);
  const dimmed = hl.dimmed.has(key);
  return {
    stroke: selected ? "#f59e0b" : e.kind === "calls" ? "#f97316" : "#94a3b8",
    lineWidth: selected ? 3.2 : 1.8,
    opacity: selected ? 0.96 : dimmed ? 0.08 : 0.5,
    lineDash: isWeakEdge(e) ? [6, 4] : undefined,
    cursor: "pointer",
  };
}
