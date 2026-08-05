// EvidenceClient — 按需证据查询（P0 §6.2 / 验收 1, 5, 7）。
//
// - preview：只读 bounded snapshot 即可回答（直接上下游、sourceRef、限制）
// - full：需要 Gateway/查询服务（完整链路、dependencyReach）
// 查询服务不可用时 full 降级为 preview 并明确标记 origin（验收 7）。
import type {
  DesktopTransport,
  EdgeEvidenceBundle,
  NodeContextBundle,
} from "../types";
import { directNeighbors, projectCoverageContext, sourceRefForNode, staticLimitations, toRelationRef } from "./snapshot-reader";
import type { SnapshotIndex } from "./snapshot-reader";

export class EvidenceClient {
  constructor(
    private readonly transport: DesktopTransport,
    private readonly index: SnapshotIndex,
    private readonly snapshotData: { limitations: unknown; graph: { summary: { callEdgeCount?: number } } },
  ) {}

  /** 选择 node 时的确定性上下文（预览即可回答，验收 1 的 100ms 内先显示）。 */
  getNodeContextPreview(nodeId: string): NodeContextBundle {
    const { upstream, downstream } = directNeighbors(this.index, nodeId, "both");
    const n = this.index.nodeById.get(nodeId);
    const ref = n ? sourceRefForNode(this.index, nodeId) : null;
    return {
      schemaVersion: "codelattice.nodeContext.v1",
      snapshotId: this.index.snapshotId,
      nodeId,
      directCallers: upstream.filter((r) => r.kind === "calls"),
      directCallees: downstream.filter((r) => r.kind === "calls"),
      sourceRefs: ref ? [ref] : [],
      limitations: staticLimitations(this.snapshotData as never),
      coverageContext: projectCoverageContext(this.snapshotData as never),
      origin: "preview",
    };
  }

  /** 选择 relation 时的确定性证据包（预览即可回答）。 */
  getEdgeEvidencePreview(relationKey: string): EdgeEvidenceBundle | null {
    const e = this.index.relationByKey.get(relationKey);
    if (!e) return null;
    const selection = toRelationRef(e);
    const src = this.index.nodeById.get(e.source);
    const tgt = this.index.nodeById.get(e.target);
    const sourceRefs = [src, tgt]
      .map((n) => (n ? sourceRefForNode(this.index, n.id) : null))
      .filter((r): r is NonNullable<typeof r> => r !== null);
    return {
      schemaVersion: "codelattice.edgeEvidence.v1",
      snapshotId: this.index.snapshotId,
      selection,
      directUpstream: directNeighbors(this.index, e.source, "upstream").upstream,
      directDownstream: directNeighbors(this.index, e.target, "downstream").downstream,
      dependencyReach: [],
      sourceRefs,
      limitations: staticLimitations(this.snapshotData as never),
      coverageContext: projectCoverageContext(this.snapshotData as never),
      generatedFrom: { staticAnalysis: true, runtimeVerified: false, coverageVerified: false },
      origin: "preview",
    };
  }

  /**
   * 完整证据查询：优先走查询服务；失败/不可用时回退到 preview，
   * 明确区分“预览”与“完整查询”（验收 5）。永不阻塞画布（验收 1）。
   */
  async getEdgeEvidenceFull(
    snapshotId: string,
    relationKey: string,
    occurrenceKey?: string,
  ): Promise<EdgeEvidenceBundle> {
    const preview = this.getEdgeEvidencePreview(relationKey);
    try {
      const full = await this.transport.getEdgeEvidence(snapshotId, relationKey, occurrenceKey);
      if (full && full.selection) return { ...full, origin: "full" };
    } catch {
      // 查询服务不可用 → 静态降级（验收 7）
    }
    if (preview) return preview;
    throw new Error(`relation not found: ${relationKey}`);
  }

  async getNodeContextFull(snapshotId: string, nodeId: string): Promise<NodeContextBundle> {
    const preview = this.getNodeContextPreview(nodeId);
    try {
      const full = await this.transport.getNodeContext(snapshotId, nodeId);
      if (full && full.nodeId) return { ...full, origin: "full" };
    } catch {
      // 静态降级
    }
    return preview;
  }

}
