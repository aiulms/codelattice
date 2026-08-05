// DashboardPanel — 确定性项目仪表盘（P0 §4.3）。
// F1 提供事实模块边界；入口/热点/结构骨架/建议起点在 P0-A 补全。
import type { SnapshotData } from "../types";
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
  limitationsCount: number;
};

export function computeDashboardFacts(
  data: Pick<SnapshotData, "graph">,
  index: SnapshotIndex,
): DashboardFacts {
  const g = data.graph;
  return {
    snapshotId: index.snapshotId,
    nodeCount: g.summary.nodeCount,
    edgeCount: g.summary.edgeCount,
    callEdgeCount: g.summary.callEdgeCount,
    symbolNodeCount: g.summary.symbolNodeCount,
    fileNodeCount: g.summary.fileNodeCount,
    truncated: g.truncated,
    resolutionRate: g.summary.callEdgeCount > 0 ? 1 : 0,
    limitationsCount: index.edges.filter((e) => e.kind === "calls").length > 0 ? 0 : 0,
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
    ["图谱截断", f.truncated ? "是（preview）" : "否"],
  ];
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
      <p className="hint">以下均为静态事实，不依赖模型服务。</p>
    </section>
  );
}
