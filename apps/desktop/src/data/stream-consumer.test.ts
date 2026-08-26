// stream-consumer 契约测试（P0-B1：chunk/complete/error/预算事件归约）。
import { describe, it, expect } from "vitest";
import {
  collectStreamEvents,
  createStreamResult,
  normalizeGatewayEvent,
  reduceStreamEvent,
  visibleAssistantText,
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

  it("accepts camelCase kinds emitted by the Rust GatewayEvent serde", () => {
    let acc = createStreamResult();
    acc = reduceStreamEvent(acc, normalizeGatewayEvent({
      kind: "answerChunk",
      text: "你好",
      requestId: "req:1",
    }), 0);
    expect(acc.text).toBe("你好");
    acc = reduceStreamEvent(acc, normalizeGatewayEvent({
      kind: "answerComplete",
      requestId: "req:1",
      answer: {
        schemaVersion: "codelattice.understandingAnswer.v1",
        scope: { type: "project", id: "p" },
        answerSummary: "总结",
        claims: [],
        navigationActions: [],
      },
    }), 1);
    expect(acc.text).toBe("总结");
  });

  it("does not throw when kind is unknown", () => {
    const out = reduceStreamEvent(
      createStreamResult(),
      normalizeGatewayEvent({ kind: "mystery", requestId: "req:1" }),
      0,
    );
    expect(out.error).toMatch(/unknown event kind/);
  });

  it("tool-call increments trace counter (B2)", () => {
    let acc = createStreamResult();
    acc = reduceStreamEvent(acc, ev("tool-call", { trace: { tool: "search_nodes", params: {}, returnedBytes: 1, truncated: false } }), 0);
    expect(acc.toolTraces).toBe(1);
  });

  it("never shows vendor tool-call XML as the assistant answer", () => {
    const xml = `<longcat_tool_call>get_node_context
<longcat_arg_key>nodeId</longcat_arg_key>
<longcat_arg_value>shell:file:build.sh</longcat_arg_value>
</longcat_tool_call>`;
    const acc = reduceStreamEvent(
      createStreamResult(),
      ev("answer-complete", {
        answer: {
          schemaVersion: "codelattice.understandingAnswer.v1",
          scope: { type: "node", id: "n" },
          answerSummary: xml,
          claims: [],
          navigationActions: [],
        },
      }),
      0,
    );
    expect(acc.text).not.toContain("longcat_tool_call");
    expect(acc.text).not.toContain("get_node_context");
    expect(acc.text.length).toBeGreaterThan(8);
    expect(visibleAssistantText(xml)).not.toContain("longcat_tool_call");
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
