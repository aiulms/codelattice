// stream-consumer — 流式事件 → 消息状态的纯 reducer（P0-B1，可独立测试）。
// 从 GatewayEvent 流中收集 chunk 文本、claims、navigationActions、错误，
// 与 UI 解耦；App 层用其渲染 ChatMessage。
import type { Claim, GatewayEvent, NavigationAction } from "../types";

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
      return { ...acc, text: acc.text + ev.text };
    case "answer-complete":
      return {
        ...acc,
        text: ev.answer.answerSummary || acc.text,
        claims: ev.answer.claims ?? [],
        navigationActions: ev.answer.navigationActions ?? [],
      };
    case "error":
      return { ...acc, error: ev.message };
    case "tool-call":
      return { ...acc, toolTraces: acc.toolTraces + 1 };
    case "budget-limit":
      return { ...acc, budgetLimit: ev.reason };
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
    const next = reduceStreamEvent(acc, ev, i);
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
