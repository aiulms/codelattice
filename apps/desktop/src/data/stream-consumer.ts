// stream-consumer — 流式事件 → 消息状态的纯 reducer（P0-B1，可独立测试）。
// 从 GatewayEvent 流中收集 chunk 文本、claims、navigationActions、错误，
// 与 UI 解耦；App 层用其渲染 ChatMessage。
import type { Claim, GatewayEvent, NavigationAction } from "../types";

const KIND_ALIASES: Record<string, GatewayEvent["kind"]> = {
  "answer-chunk": "answer-chunk",
  answerChunk: "answer-chunk",
  "answer-complete": "answer-complete",
  answerComplete: "answer-complete",
  "tool-call": "tool-call",
  toolCall: "tool-call",
  "budget-limit": "budget-limit",
  budgetLimit: "budget-limit",
  error: "error",
};

/** Rust serde camelCase（answerChunk）与前端 kebab-case（answer-chunk）对齐。 */
export function normalizeGatewayEvent(raw: { kind?: string; [key: string]: unknown }): GatewayEvent {
  const kind = KIND_ALIASES[String(raw.kind ?? "")];
  const requestId = typeof raw.requestId === "string" ? raw.requestId : "";
  if (!kind) {
    return { kind: "error", message: `unknown event kind: ${String(raw.kind)}`, requestId };
  }
  return { ...raw, kind } as GatewayEvent;
}

export function isTerminalGatewayEvent(ev: GatewayEvent): boolean {
  const kind = KIND_ALIASES[ev.kind] ?? ev.kind;
  return kind === "answer-complete" || kind === "error" || kind === "budget-limit";
}

const TOOL_MARKUP = /<\/?longcat_tool_call>|<\/?tool_call>/i;

const TOOL_FALLBACK =
  "模型想先查图上的节点，但没有把说明写出来。请再点一次「解释」，或把问题再说一遍。";

/** 龙猫等模型会把工具调用写成 XML；展示前必须剥掉，不能当正文。 */
export function stripToolCallMarkup(text: string): string {
  const block =
    /<longcat_tool_call\b[^>]*>[\s\S]*?(<\/longcat_tool_call>|$)|<tool_call\b[^>]*>[\s\S]*?(<\/tool_call>|$)/gi;
  return text.replace(block, "").replace(/\n{3,}/g, "\n\n").trim();
}

export function visibleAssistantText(text: string): string {
  const hadMarkup = TOOL_MARKUP.test(text);
  const cleaned = stripToolCallMarkup(text);
  if (cleaned) return cleaned;
  return hadMarkup ? TOOL_FALLBACK : text;
}

export type StreamResult = {
  text: string;
  claims: Claim[];
  navigationActions: NavigationAction[];
  error?: string;
  toolTraces: number;
  budgetLimit?: string;
};

export function createStreamResult(): StreamResult {
  return { text: "", claims: [], navigationActions: [], toolTraces: 0 };
}

/** 单事件归约：纯函数，不触碰 React 状态。eventIndex 保留用于未来 trace 顺序。 */
export function reduceStreamEvent(
  acc: StreamResult,
  ev: GatewayEvent,
  _eventIndex: number,
): StreamResult {
  switch (ev.kind) {
    case "answer-chunk":
      return { ...acc, text: visibleAssistantText(acc.text + (ev.text ?? "")) };
    case "answer-complete":
      return {
        ...acc,
        text: visibleAssistantText(ev.answer?.answerSummary || acc.text),
        claims: ev.answer?.claims ?? [],
        navigationActions: ev.answer?.navigationActions ?? [],
      };
    case "error":
      return { ...acc, error: ev.message };
    case "tool-call":
      return { ...acc, toolTraces: acc.toolTraces + 1 };
    case "budget-limit":
      return { ...acc, budgetLimit: ev.reason };
    default:
      return { ...acc, error: acc.error ?? `未知事件: ${String((ev as { kind?: string }).kind)}` };
  }
}

/** 消费异步事件流（iterator），返回最终结果。 */
export async function collectStreamEvents(
  events: AsyncIterable<GatewayEvent>,
  onProgress?: (acc: StreamResult, ev: GatewayEvent, index: number) => void,
): Promise<StreamResult> {
  const acc = createStreamResult();
  let i = 0;
  for await (const ev of events) {
    const next = reduceStreamEvent(acc, normalizeGatewayEvent(ev as { kind?: string; [key: string]: unknown }), i);
    // 浅拷贝替换：把最新值写回 acc 供下一次归约
    acc.text = next.text;
    acc.claims = next.claims;
    acc.navigationActions = next.navigationActions;
    acc.error = next.error;
    acc.toolTraces = next.toolTraces;
    acc.budgetLimit = next.budgetLimit;
    onProgress?.(acc, ev, i);
    i += 1;
  }
  return { ...acc };
}
