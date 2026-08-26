// 对话工作集：用户发问/解释过的图元素清单。
// 不依赖模型从长上下文里回忆；每轮把清单作为结构化事实写进 prompt。
// 存活周期跟当前后端会话绑定：换项目、重新分析、关会话则整表丢弃，不落盘。
import type { GraphSelection } from "../types";

export const WORKING_SET_CAP = 12;
const TURN_TEXT_CAP = 360;

export type WorkingSetKind = "node" | "edge" | "chain" | "multi";

export type WorkingSetItem = {
  id: string;
  kind: WorkingSetKind;
  label: string;
  block: string;
};

export type MemoryTurn = { role: "user" | "assistant"; text: string };

export function selectionWorkingId(selection: GraphSelection): string | null {
  switch (selection.type) {
    case "none":
      return null;
    case "node":
      return `node:${selection.nodeId}`;
    case "relation":
      return `edge:${selection.relationKey}`;
    case "chain":
      return `chain:${selection.chainId}`;
    case "multi":
      return `multi:${selection.nodeIds.join(",")}|${selection.relationKeys.join(",")}`;
  }
}

export function workingItemFromSelection(
  selection: GraphSelection,
  label: string,
  block: string,
): WorkingSetItem | null {
  if (selection.type === "none") return null;
  const id = selectionWorkingId(selection);
  if (!id) return null;
  const kind: WorkingSetKind = selection.type === "relation" ? "edge" : selection.type;
  return { id, kind, label, block };
}

export type ConversationEpoch = {
  sessionId: string;
  snapshotId: string;
};

/** 工作集跟当前会话走：换项目 / 新会话 / 换 snapshot 清空；首次绑定和同会话续聊保留。 */
export function conversationMemoryShouldReset(
  previous: ConversationEpoch | null,
  next: ConversationEpoch,
): boolean {
  if (previous === null) return false;
  if (!next.sessionId) return true;
  return previous.sessionId !== next.sessionId || previous.snapshotId !== next.snapshotId;
}

export function upsertWorkingSet(items: WorkingSetItem[], next: WorkingSetItem): WorkingSetItem[] {
  const rest = items.filter((item) => item.id !== next.id);
  const out = [...rest, next];
  return out.length > WORKING_SET_CAP ? out.slice(out.length - WORKING_SET_CAP) : out;
}

export function removeWorkingSetItem(items: WorkingSetItem[], id: string): WorkingSetItem[] {
  return items.filter((item) => item.id !== id);
}

export function formatWorkingSetPrompt(items: WorkingSetItem[], currentId: string | null): string {
  if (items.length === 0) return "";
  const lines = items.map((item, i) => {
    const mark = item.id === currentId ? "  ← 当前" : "";
    return `${i + 1}. ${item.block}${mark}`;
  });
  return ["[已讨论的图元素]", ...lines].join("\n");
}

export function formatRecentTurns(turns: MemoryTurn[], pairLimit = 4): string {
  const slice = turns.slice(-pairLimit * 2);
  if (slice.length === 0) return "";
  return slice.map((t) => {
    const who = t.role === "user" ? "用户" : "助手";
    const text = t.text.replace(/\s+/g, " ").trim();
    const clipped = text.length > TURN_TEXT_CAP ? `${text.slice(0, TURN_TEXT_CAP)}…` : text;
    return `${who}：${clipped}`;
  }).join("\n");
}

export function attachConversationMemory(
  userText: string,
  parts: {
    workingSetBlock: string;
    currentSelectionBlock: string | null;
    recentTurns: string;
    styleBlock?: string;
  },
): string {
  const out = [userText];
  if (parts.workingSetBlock) {
    out.push("", parts.workingSetBlock);
  }
  const skipCurrent =
    !parts.currentSelectionBlock
    || userText.includes("[当前图谱选择]")
    || userText.startsWith("请解释这条")
    || userText.startsWith("请分别解释")
    || userText.startsWith("请解释这个");
  if (!skipCurrent && parts.currentSelectionBlock) {
    out.push(
      "",
      "[当前图谱选择]",
      parts.currentSelectionBlock,
      "用户说的「这根线 / 这个节点」就是上面这一项；「两条线 / 刚才那条」指已讨论清单里的项，不要再索要 relationKey。",
    );
  }
  if (parts.recentTurns) {
    out.push("", "[最近对话]", parts.recentTurns);
  }
  if (parts.styleBlock) {
    out.push("", parts.styleBlock);
  }
  return out.join("\n");
}

export function visibleUserChatText(outbound: string, fallbackTitle?: string | null): string {
  if (
    outbound.startsWith("请解释这条")
    || outbound.startsWith("请分别解释")
    || outbound.startsWith("请解释这个")
  ) {
    return fallbackTitle ? `解释：${fallbackTitle}` : "解释当前选择";
  }
  return outbound;
}
