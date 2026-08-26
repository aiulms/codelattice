// 导览树面板：入口钉在顶部，默认只展开文件夹，点文件停在文件图。
import { useEffect, useState } from "react";
import {
  defaultOpenIds,
  type StructureGuide,
  type StructureTreeItem,
} from "../state/structure-tree";

export function StructureTreePanel(props: {
  guide: StructureGuide | null;
  selectedNodeId?: string | null;
  onJump(item: StructureTreeItem): void;
}) {
  const [openIds, setOpenIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!props.guide) {
      setOpenIds(new Set());
      return;
    }
    setOpenIds(new Set(defaultOpenIds(props.guide)));
  }, [props.guide]);

  if (!props.guide) {
    return <p className="hint">加载快照后显示结构树。</p>;
  }

  function toggle(id: string) {
    setOpenIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <div className="tree-guide" data-testid="tree-guide">
      {props.guide.startHere.length > 0 && (
        <section className="tree-start" data-testid="tree-start">
          <h3>从这里看</h3>
          <ul className="tree-start-list">
            {props.guide.startHere.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className={`tree-row tree-row-entry${props.selectedNodeId === item.nodeId ? " selected" : ""}`}
                  data-testid={`tree-entry-${item.nodeId ?? item.id}`}
                  onClick={() => props.onJump(item)}
                >
                  <span className="tree-entry-mark">入口</span>
                  <span className="tree-row-label">{item.label}</span>
                  {item.path && item.kind === "symbol" && (
                    <span className="tree-row-meta">{item.path}</span>
                  )}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <h3>项目结构</h3>
      <p className="tree-hint">先看入口和文件，函数默认收着。</p>
      <ul className="tree-outline">
        {props.guide.roots.map((item) => (
          <TreeRow
            key={item.id}
            item={item}
            openIds={openIds}
            selectedNodeId={props.selectedNodeId}
            onToggle={toggle}
            onJump={props.onJump}
          />
        ))}
      </ul>
    </div>
  );
}

function TreeRow(props: {
  item: StructureTreeItem;
  openIds: Set<string>;
  selectedNodeId?: string | null;
  onToggle(id: string): void;
  onJump(item: StructureTreeItem): void;
}) {
  const { item } = props;
  const hasKids = item.children.length > 0;
  const open = props.openIds.has(item.id);
  const selected = !!item.nodeId && item.nodeId === props.selectedNodeId;
  const testId = item.nodeId ? `tree-node-${item.nodeId}` : undefined;

  return (
    <li className={`tree-item tree-item-${item.kind}${item.isEntry ? " is-entry" : ""}`}>
      <div className={`tree-row${selected ? " selected" : ""}`}>
        {hasKids ? (
          <button
            type="button"
            className="tree-twist"
            aria-expanded={open}
            aria-label={open ? `收起${item.label}` : `展开${item.label}`}
            onClick={() => props.onToggle(item.id)}
          >
            {open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="tree-twist spacer" aria-hidden="true" />
        )}
        {item.kind === "folder" ? (
          <button
            type="button"
            className="tree-row-main"
            onClick={() => props.onToggle(item.id)}
          >
            <span className="tree-kind">{item.kindLabel}</span>
            <span className="tree-row-label">{item.label}</span>
            {/* 文件夹语言徽标 = 子孙语言去重（展示聚合，非事实陈述） */}
            {item.languages && (
              <span className="tree-kind" title={item.languages.join(" / ")}>
                {item.languages.join("/")}
              </span>
            )}
          </button>
        ) : (
          <button
            type="button"
            className="tree-row-main"
            data-testid={testId}
            onClick={() => props.onJump(item)}
          >
            <span className="tree-kind">{item.kindLabel}</span>
            <span className="tree-row-label">{item.label}</span>
            {item.language && <span className="tree-kind">{item.language}</span>}
            {item.isEntry && <span className="tree-entry-mark">入口</span>}
          </button>
        )}
      </div>
      {hasKids && open && (
        <ul>
          {item.children.map((child) => (
            <TreeRow
              key={child.id}
              item={child}
              openIds={props.openIds}
              selectedNodeId={props.selectedNodeId}
              onToggle={props.onToggle}
              onJump={props.onJump}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
