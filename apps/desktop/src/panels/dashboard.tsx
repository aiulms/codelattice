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
  moduleCount: number;
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
  data: Pick<SnapshotData, "graph" | "summary" | "limitations" | "insights" | "moduleGraph">,
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
      if (typeof ep !== "object" || ep === null || !("id" in ep)) continue;
      const rec = ep as { id: unknown; label?: unknown; name?: unknown };
      const label = rec.label ?? rec.name;
      if (label == null) continue;
      entryPoints.push({ id: String(rec.id), label: String(label) });
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

  // 三层结构骨架：模块数只读 snapshot.moduleGraph，不在前端重算边界
  const uniqueFiles = new Set(g.nodes.map((n) => n.file).filter((f): f is string => !!f));
  const fileCount = g.nodes.filter((n) => n.kind === "file").length || uniqueFiles.size;
  const symbolCount = g.nodes.filter((n) => n.kind === "symbol").length;
  const packageCount = g.nodes.filter((n) => n.kind === "package").length;
  const moduleCount = data.moduleGraph?.modules.length ?? packageCount;
  const structureSkeleton = [
    { layer: "Package / Module", count: moduleCount },
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
    moduleCount,
  };
}

export function DashboardPanel(props: {
  facts: DashboardFacts | null;
  error?: string;
  hasModuleGraph?: boolean;
  onJumpLevel?: (level: "module" | "file" | "symbol") => void;
  onJumpNode?: (nodeId: string) => void;
}) {
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
  const coverage = f.totalCalls > 0 ? `${Math.round(f.resolutionRate * 100)}%` : "N/A";
  const layerToLevel = (layer: string): "module" | "file" | "symbol" | null => {
    if (layer.startsWith("Package") || layer.startsWith("Module")) return "module";
    if (layer === "File") return "file";
    if (layer === "Symbol") return "symbol";
    return null;
  };
  return (
    <section className="panel dashboard dashboard-compact" data-testid="dashboard">
      <h2>从这里开始</h2>
      <div className="metric-row" data-testid="dashboard-metrics">
        <span className="metric-chip"><b>{f.nodeCount}</b> 节点</span>
        <span className="metric-chip"><b>{f.edgeCount}</b> 边</span>
        {f.moduleCount > 0 && <span className="metric-chip"><b>{f.moduleCount}</b> 模块</span>}
        <span className="metric-chip"><b>{f.callEdgeCount}</b> CALLS</span>
        <span className="metric-chip"><b>{coverage}</b> 覆盖</span>
      </div>

      {f.hotspots.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-hotspots">
          <h3>热点符号</h3>
          <div className="chip-list">
            {f.hotspots.map((h) => (
              <button
                key={h.id}
                type="button"
                className="jump-chip"
                onClick={() => props.onJumpNode?.(h.id)}
              >
                {h.label}
                <small>{h.callCount}</small>
              </button>
            ))}
          </div>
        </div>
      )}

      {f.structureSkeleton.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-structure">
          <h3>看哪一层</h3>
          <div className="chip-list">
            {f.structureSkeleton.map((s) => {
              const level = layerToLevel(s.layer);
              const blocked = !level || (level === "module" && !props.hasModuleGraph);
              return (
                <button
                  key={s.layer}
                  type="button"
                  className="jump-chip"
                  disabled={blocked}
                  onClick={() => level && props.onJumpLevel?.(level)}
                >
                  {s.layer}
                  <small>{s.count}</small>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {f.entryPoints.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-entry-points">
          <h3>入口点</h3>
          <div className="chip-list">
            {f.entryPoints.slice(0, 5).map((ep) => (
              <button
                key={ep.id}
                type="button"
                className="jump-chip"
                onClick={() => props.onJumpNode?.(ep.id)}
              >
                {ep.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {f.suggestions.length > 0 && (
        <div className="dashboard-section" data-testid="dashboard-suggestions">
          <h3>建议</h3>
          <ol>
            {f.suggestions.map((s, i) => (
              <li key={i}>{s}</li>
            ))}
          </ol>
        </div>
      )}

      {f.limitations.length > 0 && (
        <details className="dashboard-section" data-testid="dashboard-limitations">
          <summary>分析边界（{f.limitations.length}）</summary>
          <ul>
            {f.limitations.slice(0, 8).map((l) => (
              <li key={l.id}>{l.text}</li>
            ))}
          </ul>
        </details>
      )}

      <p className="hint">静态事实，不依赖模型。</p>
    </section>
  );
}
