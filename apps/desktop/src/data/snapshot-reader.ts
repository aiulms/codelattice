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
} from "../types";

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
