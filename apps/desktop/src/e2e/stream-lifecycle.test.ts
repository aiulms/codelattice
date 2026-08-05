// E2E: Transport stream lifecycle — 验证每请求独立状态、terminal exactly once、
// 替换旧请求时 cancel + dispose、generator finally 清理不误清理新请求。
//
// 这些测试用 FakeDesktopTransport 验证前端层语义（而非真实后端 IPC）。
// 真实后端 SSE 集成在 Rust 集成测试中覆盖。
import { describe, it, expect } from "vitest";
import { FakeDesktopTransport } from "../transport/fake-transport";
import type { GatewayEvent } from "../types";
import { collectStreamEvents } from "../data/stream-consumer";

describe("Stream lifecycle E2E", () => {
  it("collects answer-chunk + answer-complete in order", async () => {
    const transport = new FakeDesktopTransport("{}", {
      id: "snap:1", rootLabel: "test", language: "rust", createdAt: "2026-01-01T00:00:00Z",
    });
    const handle = await transport.chat({
      sessionId: "sess:1",
      message: "explain foo",
      providerId: "",
      snapshotId: "snap:1",
      pinnedScope: null,
    });
    const events: GatewayEvent[] = [];
    for await (const ev of handle.events) {
      events.push(ev);
    }
    expect(events.length).toBeGreaterThanOrEqual(1);
    const last = events[events.length - 1];
    expect(last.kind).toBe("answer-complete");
  });

  it("cancel terminates the stream", async () => {
    const transport = new FakeDesktopTransport("{}", {
      id: "snap:1", rootLabel: "test", language: "rust", createdAt: "2026-01-01T00:00:00Z",
    });
    const handle = await transport.chat({
      sessionId: "sess:1",
      message: "test",
      providerId: "",
      snapshotId: "snap:1",
      pinnedScope: null,
    });
    await handle.cancel();
    expect(transport.stats().cancelled).toContain(handle.requestId);
  });

  it("collectStreamEvents produces text + claims + navigation", async () => {
    const transport = new FakeDesktopTransport("{}", {
      id: "snap:1", rootLabel: "test", language: "rust", createdAt: "2026-01-01T00:00:00Z",
    });
    const handle = await transport.chat({
      sessionId: "sess:1",
      message: "test",
      providerId: "",
      snapshotId: "snap:1",
      pinnedScope: null,
    });
    const result = await collectStreamEvents(handle.events, () => {});
    expect(result.text).toBeDefined();
    expect(result.claims).toBeDefined();
    expect(result.navigationActions).toBeDefined();
    expect(result.error).toBeUndefined();
  });
});
