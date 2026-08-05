// G6GraphAdapter — 新 Workbench 的 G6 渲染适配层（P0-A Track A #2 / F1 #7）。
//
// 与旧 graph-g6.js 不同：edge 元素携带 relationKey（语义身份，不是布局 id），
// 并发出 edge click / edge hover 事件（F0 字符化确认的 legacy gap）。
// G6 只做渲染/交互；语义、高亮、选择所有权在 GraphController。
import type { GraphSelection, SnapshotEdge, SnapshotNode } from "../types";
import { relationKeyOf } from "../data/snapshot-reader";
import { computeHighlight } from "./highlight";

export interface G6AdapterCallbacks {
  onSelectNode(nodeId: string): void;
  onFocusNode(nodeId: string): void;
  onHoverNode(nodeId: string | null): void;
  onSelectEdge(relationKey: string): void;
  onHoverEdge(relationKey: string | null): void;
  onCanvasClick(): void;
}

export interface G6Like {
  new (config: unknown): G6GraphLike;
}
export interface G6GraphLike {
  on(event: string, fn: (evt: { target?: { id?: string }; originalEvent?: unknown }) => void): void;
  render(): Promise<void> | void;
  destroy(): void;
  setElementState(states: Record<string, string[]>): void;
  emit?: (event: string, evt: unknown) => void;
  resize?: (w: number, h: number) => void;
}

export interface RenderOptions {
  selection?: GraphSelection;
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
    this.lastSelection = opts.selection ?? { type: "none" };
    const width = Math.max(600, this.container.clientWidth || 800);
    const height = Math.max(400, this.container.clientHeight || 600);
    const hl = computeHighlight(nodes, edges, this.lastSelection);

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
        style: {
          stroke: hl.selected.has(key) ? "#f59e0b" : e.kind === "calls" ? "#f97316" : "#94a3b8",
          lineWidth: hl.selected.has(key) ? 3.2 : 1.2,
          opacity: hl.selected.has(key) ? 0.96 : hl.dimmed.has(key) ? 0.055 : 0.44,
        },
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
          labelText: n.label.slice(0, 30),
          labelFill: "#172033",
        },
      };
    });

    try {
      this.destroy();
      const graph = new this.g6Factory({
        container: this.container,
        width,
        height,
        autoResize: true,
        data: { nodes: g6Nodes, edges: g6Edges },
        behaviors: ["drag-canvas", "zoom-canvas", "drag-element"],
      });
      graph.on("node:click", (evt) => {
        const id = evt.target?.id;
        if (id) this.callbacks.onSelectNode(id);
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
        if (key) this.callbacks.onSelectEdge(key);
      });
      graph.on("edge:pointerenter", (evt) => {
        const key = evt.target?.id ? this.edgeElementToRelation.get(evt.target.id) : undefined;
        if (key) this.callbacks.onHoverEdge(key);
      });
      graph.on("edge:pointerleave", () => this.callbacks.onHoverEdge(null));
      graph.on("canvas:click", () => this.callbacks.onCanvasClick());
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
    if (this.graph) this.lastSelection = selection;
    // 高亮差异通过重渲染应用（bounded snapshot 规模下成本可接受）
    this.render(this.lastNodes, this.lastEdges, { selection });
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
  }
}
