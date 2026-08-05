// DashboardPanel — 确定性项目仪表盘（P0 §4.3 / 返工修复：真实数据）。
// 返工修复：
// - resolutionRate 从 snapshot summary 真实计算（不再伪计算为 1）
// - 增加入口点、热点符号、三层结构骨架、CALLS coverage、静态限制、建议起点
// - 所有指标来自真实 snapshot/query store
import type { SnapshotData, StaticLimitation } from "../types";
import type { SnapshotIndex } from "../data/snapshot-reader";

export type DashboardFacts = {
  snapshotId: string;
  nodeCount: number;
  edgeCount: number;
  callEdgeCount: number;
  symbolNodeCount: number;
  fileNodeCount: number;
  truncated: boolean;
  resolutionRate: number;
  totalCalls: number;
  limitationsCount: number;
  limitations: StaticLimitation[];
  entryPoints: Array<{ id: string; label: string }>;
  hotspots: Array<{ id: string; label: string; callCount: number }>;
  structureSkeleton: Array<{ layer: string; count: number }>;
  suggestions: string[];
};

interface SnapshotSummary {
  resolvedCalls?: number;
  totalCalls?: number;
  callEdgeCount?: number;
  unresolvedCalls?: number;
}

function safeSummary(data: Pick<SnapshotData, "summary">): SnapshotSummary {
  return (data.summary as Record<string, unknown>) as SnapshotSummary;
}

export function computeDashboardFacts(
  data: Pick<SnapshotData, "graph" | "summary" | "limitations" | "insights">,
  index: SnapshotIndex,
): DashboardFacts {
  const g = data.graph;
  const s = safeSummary(data);

  // 返工修复：从 summary 真实计算 resolutionRate
  const resolved = s.resolvedCalls ?? 0;
  const total = s.totalCalls ?? s.callEdgeCount ?? g.summary.callEdgeCount;
  const resolutionRate = total > 0 ? resolved / total : 0;

  // 静态限制
  const limitationsRaw = data.limitations;
  let limitations: StaticLimitation[] = [];
  if (Array.isArray(limitationsRaw)) {
    limitations = limitationsRaw.map((text, i) => ({ id: `limit:${i}`, text }));
  } else if (limitationsRaw && typeof limitationsRaw === "object" && "notes" in limitationsRaw) {
    const notes = (limitationsRaw as { notes: string[] }).notes;
    limitations = notes.map((text, i) => ({ id: `limit:${i}`, text }));
  }

  // 入口点：从 snapshot.insights.entryPoints 或 entry kind 节点
  const entryPoints: Array<{ id: string; label: string }> = [];
  if (data.insights?.entryPoints && Array.isArray(data.insights.entryPoints)) {
    for (const ep of data.insights.entryPoints) {
      if (typeof ep === "object" && ep !== null && "id" in ep && "label" in ep) {
        entryPoints.push({ id: String(ep.id), label: String(ep.label) });
      }
    }
  }
  if (entryPoints.length === 0) {
    for (const n of g.nodes) {
      if (n.kind === "entry") entryPoints.push({ id: n.id, label: n.label });
    }
  }

  // 热点符号：按出入度排序取 top-5
  const degreeMap = new Map<string, number>();
  for (const e of g.edges) {
    if (e.kind === "calls") {
      degreeMap.set(e.source, (degreeMap.get(e.source) ?? 0) + 1);
      degreeMap.set(e.target, (degreeMap.get(e.target) ?? 0) + 1);
    }
  }
  const hotspots = Array.from(degreeMap.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([id, callCount]) => {
      const node = g.nodes.find((n) => n.id === id);
      return { id, label: node?.label ?? id, callCount };
    });

  // 三层结构骨架
  const fileCount = g.nodes.filter((n) => n.kind === "file").length;
  const symbolCount = g.nodes.filter((n) => n.kind === "symbol").length;
  const packageCount = g.nodes.filter((n) => n.kind === "package").length;
  const structureSkeleton = [
    { layer: "Package / Module", count: packageCount },
    { layer: "File", count: fileCount },
    { layer: "Symbol", count: symbolCount },
  ];

  // 建议起点
  const suggestions: string[] = [];
  if (entryPoints.length > 0) {
    suggestions.push(`从入口点 "${entryPoints[0].label}" 开始浏览`);
  }
  if (hotspots.length > 0) {
    suggestions.push(`查看高频调用节点 "${hotspots[0].label}"（${hotspots[0].callCount} 次调用）`);
  }
  if (limitations.length > 0) {
    suggestions.push(`了解静态分析限制（${limitations.length} 条注意事项）`);
  }
  if (suggestions.length === 0) {
    suggestions.push("从图谱中任意节点开始探索");
  }

  return {
    snapshotId: index.snapshotId,
    nodeCount: g.summary.nodeCount,
    edgeCount: g.summary.edgeCount,
    callEdgeCount: g.summary.callEdgeCount,
    symbolNodeCount: g.summary.symbolNodeCount,
    fileNodeCount: g.summary.fileNodeCount,
    truncated: g.truncated,
    resolutionRate,
    totalCalls: total,
    limitationsCount: limitations.length,
    limitations,
    entryPoints,
    hotspots,
    structureSkeleton,
    suggestions,
  };
}

export function DashboardPanel(props: { facts: DashboardFacts | null; error?: string }) {
  const f = props.facts;
  if (props.error) {
    return (
      <section className="panel dashboard" data-testid="dashboard-error">
        <h2>仪表盘</h2>
        <p className="error-text">{props.error}</p>
      </section>
    );
  }
  if (!f) {
    return (
      <section className="panel dashboard" data-testid="dashboard-empty">
        <h2>仪表盘</h2>
        <p>加载快照后可查看确定性项目事实。</p>
      </section>
    );
  }
  const rows: Array<[string, string | number]> = [
    ["快照", f.snapshotId.slice(0, 24)],
    ["节点 / 边", `${f.nodeCount} / ${f.edgeCount}`],
    ["符号节点", f.symbolNodeCount],
    ["文件节点", f.fileNodeCount],
    ["CALLS 边", f.callEdgeCount],
  ];
  // 覆盖率：totalCalls 未知时不显示 0%，改为 N/A
  if (f.totalCalls > 0) {
    rows.push(["CALLS 覆盖率", `${Math.round(f.resolutionRate * 100)}%`]);
  }
  rows.push(
    ["图谱截断", f.truncated ? "是（preview）" : "否"],
    ["静态限制", f.limitationsCount],
  );
  return (
    <section className="panel dashboard" data-testid="dashboard">
      <h2>项目事实</h2>
      <table className="fact-table">
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k}>
              <td>{k}</td>
              <td>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {f.entryPoints.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-entry-points">
          <h3>入口点（{f.entryPoints.length}）</h3>
          <ul>
            {f.entryPoints.slice(0, 5).map((ep) => (
              <li key={ep.id}>{ep.label}</li>
            ))}
          </ul>
        </div>
      )}

      {f.hotspots.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-hotspots">
          <h3>热点符号</h3>
          <ul>
            {f.hotspots.map((h) => (
              <li key={h.id}>{h.label}（{h.callCount} 次调用）</li>
            ))}
          </ul>
        </div>
      )}

      {f.structureSkeleton.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-structure">
          <h3>结构骨架</h3>
          <ul>
            {f.structureSkeleton.map((s) => (
              <li key={s.layer}>{s.layer}：{s.count}</li>
            ))}
          </ul>
        </div>
      )}

      {f.limitations.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-limitations">
          <h3>静态限制（{f.limitations.length}）</h3>
          <ul>
            {f.limitations.slice(0, 5).map((l) => (
              <li key={l.id}>{l.text}</li>
            ))}
          </ul>
        </div>
      )}

      {f.suggestions.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-suggestions">
          <h3>建议起点</h3>
          <ol>
            {f.suggestions.map((s, i) => (
              <li key={i}>{s}</li>
            ))}
          </ol>
        </div>
      )}

      <p className="hint">以上均为静态事实，不依赖模型服务。</p>
    </section>
  );
}
