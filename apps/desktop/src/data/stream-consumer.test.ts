// stream-consumer 契约测试（P0-B1：chunk/complete/error/预算事件归约）。
import { describe, it, expect } from "vitest";
import {
  collectStreamEvents,
  createStreamResult,
  reduceStreamEvent,
} from "./stream-consumer";
import type { GatewayEvent } from "../types";

function ev(kind: GatewayEvent["kind"], partial: Record<string, unknown>): GatewayEvent {
  return { kind, requestId: "req:1", ...(partial as object) } as GatewayEvent;
}

describe("reduceStreamEvent", () => {
  it("chunks concatenate text in order", () => {
    let acc = createStreamResult();
    acc = reduceStreamEvent(acc, ev("answer-chunk", { text: "你好" }), 0);
    acc = reduceStreamEvent(acc, ev("answer-chunk", { text: "世界" }), 1);
    expect(acc.text).toBe("你好世界");
  });

  it("complete replaces text with summary and carries claims/navigation", () => {
    const acc = createStreamResult();
    const answer = {
      schemaVersion: "codelattice.understandingAnswer.v1",
      scope: { type: "project", id: "p" },
      answerSummary: "总结",
      claims: [{ id: "claim:1", text: "x", classification: "hypothesis", evidenceRefs: [], coverageCaveatRefs: [] }],
      navigationActions: [{ type: "focusRelation", relationKey: "rel:abc", snapshotId: "s" }],
    };
    const out = reduceStreamEvent(acc, ev("answer-complete", { answer }), 0);
    expect(out.text).toBe("总结");
    expect(out.claims).toHaveLength(1);
    expect(out.navigationActions).toHaveLength(1);
  });

  it("error and budget-limit are captured", () => {
    let acc = createStreamResult();
    acc = reduceStreamEvent(acc, ev("error", { message: "timeout" }), 0);
    expect(acc.error).toBe("timeout");
    acc = reduceStreamEvent(acc, ev("budget-limit", { reason: "evidence tokens exhausted" }), 1);
    expect(acc.budgetLimit).toContain("exhausted");
  });

  it("tool-call increments trace counter (B2)", () => {
    let acc = createStreamResult();
    acc = reduceStreamEvent(acc, ev("tool-call", { trace: { tool: "search_nodes", params: {}, returnedBytes: 1, truncated: false } }), 0);
    expect(acc.toolTraces).toBe(1);
  });
});

describe("collectStreamEvents", () => {
  it("collects full stream and reports progress", async () => {
    const events: GatewayEvent[] = [
      ev("answer-chunk", { text: "逐步" }),
      ev("answer-chunk", { text: "生成" }),
      ev("answer-complete", {
        answer: {
          schemaVersion: "codelattice.understandingAnswer.v1",
          scope: { type: "node", id: "n:a" },
          answerSummary: "最终回答",
          claims: [],
          navigationActions: [],
        },
      }),
    ];
    async function* gen() {
      for (const e of events) yield e;
    }
    let progressCalls = 0;
    const result = await collectStreamEvents(gen(), () => {
      progressCalls += 1;
    });
    expect(result.text).toBe("最终回答");
    expect(progressCalls).toBe(3);
  });
});
