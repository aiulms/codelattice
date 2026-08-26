// InspectorPanel — 事实检查器（P0 §4.1 / P0-A #7）。
// F1 提供模块边界与事实展示骨架；直接上下游/coverageContext/证据在 P0-A 补全。
import type { EdgeEvidenceBundle, GraphSelection, NodeContextBundle } from "../types";
import type { AggregateFacts } from "../data/snapshot-reader";

export function InspectorPanel(props: {
  selection: GraphSelection;
  nodeContext: NodeContextBundle | null;
  edgeEvidence: EdgeEvidenceBundle | null;
  aggregateFacts?: AggregateFacts | null;
  onExplainClick(): void;
  onJoinConversation(): void;
}) {
  if (props.selection.type === "none") {
    return (
      <aside className="panel inspector" data-testid="inspector">
        <h2>检查器</h2>
        <p>未选择节点或边。选择后此处显示事实。</p>
      </aside>
    );
  }

  const sel = props.selection;
  return (
    <aside className="panel inspector" data-testid="inspector">
      <h2>检查器</h2>
      {!props.aggregateFacts && (
        <p className="selection-line">
          {sel.type === "node" && <>节点：{sel.nodeId}</>}
          {sel.type === "relation" && <>关系：{sel.relationKey}</>}
          {sel.type === "chain" && <>链路：{sel.chainId}</>}
          {sel.type === "multi" && <>多选：{sel.nodeIds.length + sel.relationKeys.length} 项</>}
        </p>
      )}

      {props.aggregateFacts && (
        <div className="inspector-facts" data-testid="inspector-aggregate-facts">
          <p className="inspector-kicker">{props.aggregateFacts.kicker}</p>
          <p className="inspector-title">{props.aggregateFacts.title}</p>
          <table className="fact-table">
            <tbody>
              {props.aggregateFacts.rows.map(([k, v], i) => (
                <tr key={`${k}-${i}`}>
                  <td>{k}</td>
                  <td>{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="hint">已有边向上归并，不是新推断的依赖。</p>
        </div>
      )}

      {!props.aggregateFacts && sel.type === "node" && props.nodeContext && (
        <div className="inspector-facts" data-testid="inspector-node-facts">
          <h3>直接调用方（{props.nodeContext.directCallers.length}）</h3>
          <ul>
            {props.nodeContext.directCallers.map((r) => (
              <li key={r.relationKey}>
                {r.sourceId} → <em>{r.kind}</em>
              </li>
            ))}
          </ul>
          <h3>直接被调方（{props.nodeContext.directCallees.length}）</h3>
          <ul>
            {props.nodeContext.directCallees.map((r) => (
              <li key={r.relationKey}>
                {r.targetId} → <em>{r.kind}</em>
              </li>
            ))}
          </ul>
          <p className="hint">origin: {props.nodeContext.origin}</p>
        </div>
      )}

      {!props.aggregateFacts && sel.type === "relation" && props.edgeEvidence && (
        <div className="inspector-facts" data-testid="inspector-edge-facts">
          <p>
            {props.edgeEvidence.selection.sourceId} → {props.edgeEvidence.selection.targetId}
            （{props.edgeEvidence.selection.kind}）
          </p>
          <p className="hint">
            覆盖率：{props.edgeEvidence.coverageContext.resolvedCalls}/{props.edgeEvidence.coverageContext.totalCalls}
            {" "}（{Math.round(props.edgeEvidence.coverageContext.resolutionRate * 100)}%，project scope）
          </p>
          <p className="hint">origin: {props.edgeEvidence.origin}</p>
        </div>
      )}

      <div className="inspector-actions">
        <button type="button" onClick={props.onExplainClick}>解释当前选择</button>
        <button type="button" onClick={props.onJoinConversation}>加入对话</button>
      </div>
    </aside>
  );
}
