// SnapshotReader — bounded snapshot 静态读取（P0 §4.4 / §5.2 Static Snapshot Reader）。
//
// 只消费 webui.snapshot.v1 的 graph section；直接关系、仪表盘和静态限制在
// Runner/Gateway 不可用时仍可浏览（验收 7）。完整链路由 EvidenceClient
// 按需查询，不预埋进 snapshot（验收 5）。
import type {
  SnapshotData,
  SnapshotEdge,
  SnapshotNode,
  CoverageContext,
  RelationRef,
  SourceRef,
  StaticLimitation,
  GraphLevel,
  GraphSelection,
  ModuleGraph,
} from "../types";

export type GraphView = {
  level: GraphLevel;
  nodes: SnapshotNode[];
  edges: SnapshotEdge[];
};

/** 按层级取出可渲染图。模块边界不在前端重算，只消费 snapshot.moduleGraph。 */
export function graphViewForLevel(data: SnapshotData, level: GraphLevel): GraphView {
  if (level === "module") {
    if (!data.moduleGraph) return { level: "symbol", nodes: data.graph.nodes, edges: data.graph.edges };
    return moduleGraphToView(data.moduleGraph);
  }
  if (level === "file") return aggregateByFile(data.graph.nodes, data.graph.edges);
  return { level: "symbol", nodes: data.graph.nodes, edges: data.graph.edges };
}

function moduleGraphToView(mg: ModuleGraph): GraphView {
  const nodes: SnapshotNode[] = mg.modules.map((m) => ({
    id: `mod:${m.id}`,
    label: m.id,
    kind: "package",
    file: m.id,
  }));
  const edges: SnapshotEdge[] = mg.edges.map((e) => ({
    source: `mod:${e.source}`,
    target: `mod:${e.target}`,
    kind: (e.kinds[0] ?? "related") as SnapshotEdge["kind"],
    confidence: e.minConfidence,
    reason: e.reasons?.[0],
    count: e.count,
    relationKey: `modrel:${e.source}\u0000${e.target}`,
  }));
  return { level: "module", nodes, edges };
}

function fileKeyOf(n: SnapshotNode): string {
  if (n.kind === "file") return n.file || n.label || n.id;
  return n.file || "";
}

/** 文件级同样只归并已有跨文件边，不新造关系。 */
function aggregateByFile(nodes: SnapshotNode[], edges: SnapshotEdge[]): GraphView {
  const fileOf = new Map(nodes.map((n) => [n.id, fileKeyOf(n)]));
  const files = new Map<string, SnapshotNode>();
  for (const n of nodes) {
    const key = fileOf.get(n.id) || "";
    if (!key) continue;
    if (!files.has(key)) {
      files.set(key, { id: `file:${key}`, label: key, kind: "file", file: key });
    }
  }
  const agg = new Map<string, SnapshotEdge>();
  for (const e of edges) {
    const a = fileOf.get(e.source) || "";
    const b = fileOf.get(e.target) || "";
    if (!a || !b || a === b) continue;
    const id = `${a}\u0000${b}`;
    const prev = agg.get(id);
    if (prev) {
      prev.count = (prev.count ?? 1) + 1;
      if (typeof e.confidence === "number") {
        prev.confidence = prev.confidence == null ? e.confidence : Math.min(prev.confidence, e.confidence);
      }
    } else {
      agg.set(id, {
        source: `file:${a}`,
        target: `file:${b}`,
        kind: e.kind,
        confidence: e.confidence,
        reason: e.reason,
        count: 1,
        relationKey: `filerel:${a}\u0000${b}`,
      });
    }
  }
  return { level: "file", nodes: [...files.values()], edges: [...agg.values()] };
}

export type AggregateFacts = {
  kicker: string;
  title: string;
  rows: Array<[string, string]>;
};

/** 模块/文件视图上的选择 id，符号级 evidence 查不到。 */
export function isAggregateElementId(id: string): boolean {
  return id.startsWith("mod:") || id.startsWith("file:") || id.startsWith("modrel:") || id.startsWith("filerel:");
}

function stripViewPrefix(id: string): string {
  return id.replace(/^(mod|file):/, "");
}

/** 聚合选择的检查器事实：只复述 moduleGraph / 文件归并结果，不重算模块边界。 */
export function aggregateSelectionFacts(
  data: SnapshotData,
  level: GraphLevel,
  selection: GraphSelection,
): AggregateFacts | null {
  if (selection.type === "multi") {
    const view = graphViewForLevel(data, level);
    const rows: Array<[string, string]> = [];
    selection.nodeIds.forEach((id, i) => {
      const n = view.nodes.find((x) => x.id === id);
      rows.push([`节点 ${i + 1}`, n?.label ?? id]);
    });
    selection.relationKeys.forEach((key, i) => {
      const e = view.edges.find((x) => relationKeyOf(x) === key);
      const label = e
        ? `${stripViewPrefix(e.source)} → ${stripViewPrefix(e.target)} · ${e.kind} · 底层 ${e.count ?? 1} · 最弱置信 ${e.confidence ?? "—"}`
        : key;
      rows.push([`边 ${i + 1}`, label]);
    });
    if (rows.length === 0) return null;
    return { kicker: "多选", title: `${rows.length} 项`, rows };
  }
  if (level === "symbol" || selection.type === "none" || selection.type === "chain") return null;
  const view = graphViewForLevel(data, level);
  const kicker = level === "module" ? "模块" : "文件";
  if (selection.type === "node") {
    const n = view.nodes.find((x) => x.id === selection.nodeId);
    if (!n) return null;
    const outgoing = view.edges.filter((e) => e.source === n.id);
    const incoming = view.edges.filter((e) => e.target === n.id);
    const sum = (es: SnapshotEdge[]) => es.reduce((s, e) => s + (e.count ?? 1), 0);
    const rows: Array<[string, string]> = [];
    if (level === "module" && data.moduleGraph) {
      const m = data.moduleGraph.modules.find((x) => x.id === stripViewPrefix(n.id));
      if (m) {
        rows.push(["文件数", String(m.files)], ["符号数", String(m.symbols)]);
      }
    }
    rows.push(["出边（底层）", String(sum(outgoing))], ["入边（底层）", String(sum(incoming))]);
    return { kicker, title: n.label, rows };
  }
  const e = view.edges.find((x) => relationKeyOf(x) === selection.relationKey);
  if (!e) return null;
  return {
    kicker: "聚合边",
    title: `${stripViewPrefix(e.source)} → ${stripViewPrefix(e.target)}`,
    rows: [
      ["底层边数", String(e.count ?? 1)],
      ["类型", e.kind],
      ["最弱置信", e.confidence != null ? String(e.confidence) : "—"],
      ["原因", e.reason ?? "—"],
    ],
  };
}

/** 给 Chat 用的可读选择摘要；模型看不到图上的高亮，必须把这项写进消息。 */
export function formatSelectionForChat(
  data: SnapshotData,
  level: GraphLevel,
  selection: GraphSelection,
): string | null {
  if (selection.type === "none") return null;
  const facts = aggregateSelectionFacts(data, level, selection);
  if (facts) {
    return [`${facts.kicker}：${facts.title}`, ...facts.rows.map(([k, v]) => `${k}：${v}`)].join("\n");
  }
  if (selection.type === "node") return `节点：${selection.nodeId}`;
  if (selection.type === "relation") return `关系：${selection.relationKey}`;
  if (selection.type === "chain") return `链路：${selection.chainId}`;
  if (selection.type === "multi") {
    return `多选：${selection.nodeIds.length} 个节点，${selection.relationKeys.length} 条边`;
  }
  return null;
}

export function selectionHeadline(facts: AggregateFacts | null, selection: GraphSelection): string | null {
  if (selection.type === "none") return null;
  if (facts) return `${facts.kicker} ${facts.title}`;
  if (selection.type === "node") return selection.nodeId;
  if (selection.type === "relation") return "已选中一条边";
  if (selection.type === "chain") return selection.chainId;
  if (selection.type === "multi") return `多选 ${selection.nodeIds.length + selection.relationKeys.length} 项`;
  return null;
}

/** 自由提问时附带当前选择；解释类 prompt 已经自带事实，不再重复。 */
export function attachSelectionToChatMessage(userText: string, selectionBlock: string | null): string {
  if (!selectionBlock) return userText;
  if (
    userText.includes("[当前图谱选择]")
    || userText.startsWith("请解释这条")
    || userText.startsWith("请分别解释")
  ) {
    return userText;
  }
  return [
    userText,
    "",
    "[当前图谱选择]",
    selectionBlock,
    "用户说的「这根线 / 这个节点」就是上面这一项，不要再索要 relationKey。",
  ].join("\n");
}

export interface SnapshotIndex {
  snapshotId: string;
  nodes: SnapshotNode[];
  edges: SnapshotEdge[];
  nodeById: Map<string, SnapshotNode>;
  relationByKey: Map<string, SnapshotEdge>;
  /** source -> edges */
  outEdges: Map<string, SnapshotEdge[]>;
  /** target -> edges */
  inEdges: Map<string, SnapshotEdge[]>;
}

export function relationKeyOf(edge: SnapshotEdge): string {
  return edge.relationKey ?? defaultRelationKey(edge.source, edge.kind, edge.target);
}

/** §6.1 初始规则：relationKey = sha256(source + kind + target)。 */
export function defaultRelationKey(source: string, kind: string, target: string): string {
  return `rel:sha256:${sha256Hex(`${source}\u0000${kind}\u0000${target}`)}`;
}

export function sha256Hex(s: string): string {
  // 生产 relationKey 由 snapshot 生成器（Python hashlib.sha256）产出；
  // 前端此函数只用于旧 snapshot（无 relationKey）的同步确定性回退 key。
  // WebCrypto 的 sha256 是异步的，这里用 FNV-1a 64 位保持同步、确定性，
  // 仅作为稳定身份用，不做安全承诺。
  return fnv1aHex(s);
}

export function fnv1aHex(s: string): string {
  let h = 0xcbf29ce484222325n;
  for (let i = 0; i < s.length; i++) {
    h ^= BigInt(s.charCodeAt(i));
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return h.toString(16).padStart(16, "0");
}

export function buildIndex(data: SnapshotData, snapshotId = data.generatedAt): SnapshotIndex {
  const nodeById = new Map<string, SnapshotNode>();
  for (const n of data.graph.nodes) nodeById.set(n.id, n);
  const edges = data.graph.edges;
  const relationByKey = new Map<string, SnapshotEdge>();
  for (const e of edges) relationByKey.set(relationKeyOf(e), e);
  const outEdges = new Map<string, SnapshotEdge[]>();
  const inEdges = new Map<string, SnapshotEdge[]>();
  for (const e of edges) {
    (outEdges.get(e.source) ?? outEdges.set(e.source, []).get(e.source)!).push(e);
    (inEdges.get(e.target) ?? inEdges.set(e.target, []).get(e.target)!).push(e);
  }
  return { snapshotId, nodes: data.graph.nodes, edges, nodeById, relationByKey, outEdges, inEdges };
}

/** 直接上下游（预览语义：只含 bounded snapshot 内的边，验收 1/5）。 */
export function directNeighbors(
  idx: SnapshotIndex,
  nodeId: string,
  direction: "both" | "upstream" | "downstream" = "both",
): { upstream: RelationRef[]; downstream: RelationRef[] } {
  const upstream: RelationRef[] = [];
  const downstream: RelationRef[] = [];
  for (const e of idx.inEdges.get(nodeId) ?? []) {
    upstream.push(toRelationRef(e));
  }
  for (const e of idx.outEdges.get(nodeId) ?? []) {
    downstream.push(toRelationRef(e));
  }
  if (direction === "upstream") return { upstream, downstream: [] };
  if (direction === "downstream") return { upstream: [], downstream };
  return { upstream, downstream };
}

export function toRelationRef(e: SnapshotEdge): RelationRef {
  return {
    relationKey: relationKeyOf(e),
    occurrenceKey: e.occurrenceKey ?? null,
    sourceId: e.source,
    targetId: e.target,
    kind: e.kind,
  };
}

/** sourceRef 从 node/edge 可得的文件+行信息；snapshot 内无 span 时为尽力而为。 */
export function sourceRefForNode(idx: SnapshotIndex, nodeId: string): SourceRef | null {
  const n = idx.nodeById.get(nodeId);
  if (!n || !n.file) return null;
  return { id: `src:${nodeId}`, file: n.file, startLine: n.line ?? 0, endLine: n.line ?? 0 };
}

export function staticLimitations(data: SnapshotData): StaticLimitation[] {
  const raw = data.limitations;
  const notes = Array.isArray(raw) ? raw : (raw as { notes?: string[] }).notes ?? [];
  return notes.map((text, i) => ({ id: `limit:${i}`, text }));
}

/** 项目级 coverageContext（验收 8）：只使用事实层统计，不推算模块覆盖率。 */
export function projectCoverageContext(data: SnapshotData): CoverageContext {
  const g = data.graph.summary;
  // bounded snapshot 只有 callEdgeCount（resolved）；总调用数在 snapshot 内
  // 不可得时，knownIncomplete=true 且不伪造分母。
  const resolvedCalls = g.callEdgeCount ?? 0;
  const totalCalls = resolvedCalls; // snapshot 层无法给出 total -> 不虚构差异
  return {
    scope: "project",
    resolvedCalls,
    totalCalls,
    resolutionRate: totalCalls > 0 ? resolvedCalls / totalCalls : 0,
    knownIncomplete: true,
    caveatRef: "coverage:project:calls",
  };
}
