// ProjectPickerPanel — 体检结果挑选器（多语言卡 3）。
//
// 主列表 = projects ∪ sourceOnlyAreas 全部行：analyzable 可点，其余灰显附 reason；
// unsupportedAreas 默认折叠（真实仓库 unrecognized 行极多，不折叠会刷屏），展开只读。
// 语言信息全部来自 inspect 信封；本组件不做任何检测或 feature 判定。
import { useState } from "react";
import type { InspectionEvidence, InspectionRow, WorkspaceInspection } from "../types";

/** 证据摘要：manifest 文件名或扩展名直方图。 */
function evidenceSummary(evidence: InspectionEvidence): string {
  if (evidence.kind === "manifest") return evidence.file ?? "manifest";
  if (evidence.kind === "extension-histogram") {
    return `${evidence.extension ?? "?"} × ${evidence.count ?? 0}`;
  }
  return "";
}

function PickerRow(props: { row: InspectionRow; onSelect(row: InspectionRow): void }) {
  const { row } = props;
  const testId = `picker-row-${row.relativePath}-${row.language ?? "unknown"}`;
  const title = row.name ?? row.relativePath;
  if (!row.analyzable) {
    // 灰显不可点：没编对应 feature 的语言区（如 python），如实展示 reason
    return (
      <li className="picker-row disabled" data-testid={testId}>
        <span className="picker-row-name">{title}</span>
        <span className="picker-row-lang">{row.language ?? "—"}</span>
        <span className="picker-row-count">{row.sourceFileCount} 文件</span>
        <span className="picker-row-evidence">{evidenceSummary(row.evidence)}</span>
        <span className="picker-row-reason">{row.reason ?? "不可分析"}</span>
      </li>
    );
  }
  return (
    <li className="picker-row">
      <button type="button" data-testid={testId} onClick={() => props.onSelect(row)}>
        <span className="picker-row-name">{title}</span>
        <span className="picker-row-lang">{row.language ?? "—"}</span>
        <span className="picker-row-count">{row.sourceFileCount} 文件</span>
        <span className="picker-row-evidence">{evidenceSummary(row.evidence)}</span>
      </button>
    </li>
  );
}

export function ProjectPickerPanel(props: {
  inspection: WorkspaceInspection;
  onSelect(row: InspectionRow): void;
  onCancel(): void;
}) {
  const [unsupportedOpen, setUnsupportedOpen] = useState(false);
  const mainRows = [...props.inspection.projects, ...props.inspection.sourceOnlyAreas];
  return (
    <section className="project-picker" data-testid="project-picker" aria-label="选择要分析的项目">
      <header className="picker-head">
        <div>
          <span className="settings-kicker">PICK A PROJECT</span>
          <h3>选择要分析的项目</h3>
          <p className="picker-hint">检测到多个可分析候选；一次只分析一个项目。</p>
        </div>
        <button type="button" data-testid="picker-cancel" onClick={props.onCancel}>
          取消
        </button>
      </header>

      <ul className="picker-list">
        {mainRows.map((row) => (
          <PickerRow key={`${row.relativePath}:${row.language ?? "?"}`} row={row} onSelect={props.onSelect} />
        ))}
      </ul>

      {props.inspection.unsupportedAreas.length > 0 && (
        <div className="picker-unsupported" data-testid="picker-unsupported">
          <button
            type="button"
            className="picker-unsupported-toggle"
            aria-expanded={unsupportedOpen}
            onClick={() => setUnsupportedOpen((v) => !v)}
          >
            {unsupportedOpen ? "▾" : "▸"} 暂不支持的区域 · {props.inspection.unsupportedAreas.length}
          </button>
          {unsupportedOpen && (
            <ul className="picker-unsupported-list">
              {props.inspection.unsupportedAreas.map((row) => (
                <li key={`${row.relativePath}:${row.language ?? row.recognition ?? "?"}`}>
                  <span className="picker-row-name">{row.relativePath}</span>
                  <span className="picker-row-lang">{row.language ?? row.recognition ?? "—"}</span>
                  <span className="picker-row-count">{row.sourceFileCount} 文件</span>
                  <span className="picker-row-evidence">{evidenceSummary(row.evidence)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
