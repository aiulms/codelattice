// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { WorkbenchApp } from "../App";
import { ChatPanel } from "../panels/chat";
import { FakeDesktopTransport } from "../transport/fake-transport";
import type { ChatRequest, StreamHandle } from "../types";

vi.mock("../panels/graph-pane", () => ({
  GraphPane: () => <section data-testid="graph-pane-test-double" />,
}));

const here = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.resolve(
  here,
  "../../../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json",
);

afterEach(() => cleanup());

class SlowChatTransport extends FakeDesktopTransport {
  cancelledRequestIds: string[] = [];

  override async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return { default: "mock", models: [{ id: "mock" }] };
  }

  override async chat(_req: ChatRequest): Promise<StreamHandle> {
    const requestId = "req:slow-chat";
    return {
      requestId,
      events: (async function* () {
        await new Promise<void>(() => {});
      })(),
      cancel: async () => {
        this.cancelledRequestIds.push(requestId);
      },
    };
  }
}

class SessionRecordingTransport extends FakeDesktopTransport {
  createdForSnapshotIds: string[] = [];

  override async sessionCreate(snapshotId: string): Promise<string> {
    this.createdForSnapshotIds.push(snapshotId);
    return `sess:${this.createdForSnapshotIds.length}`;
  }
}

describe("Workbench production behavior", () => {
  it("creates the backend session with the stable snapshot-library id", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const transport = new SessionRecordingTransport(json, {
      id: "rust-portable-smoke.snapshot",
      rootLabel: "test",
      language: "rust",
      createdAt: "2026-08-05T00:00:00Z",
    });

    render(<WorkbenchApp transport={transport} />);

    await waitFor(() => {
      expect(transport.createdForSnapshotIds).toEqual(["rust-portable-smoke.snapshot"]);
    });
  });

  it("Stop generation invokes cancel on the active stream handle", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new SlowChatTransport(json, {
      id: "snap:test",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);

    const input = await screen.findByTestId("chat-input");
    await waitFor(() => expect((input as HTMLInputElement).disabled).toBe(false));
    fireEvent.change(input, { target: { value: "解释项目" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    fireEvent.click(await screen.findByTestId("chat-cancel"));

    await waitFor(() => {
      expect(transport.cancelledRequestIds).toEqual(["req:slow-chat"]);
    });
  });

  it("evidence relation chip is a navigation button", () => {
    const onNavigate = vi.fn();
    render(
      <ChatPanel
        context={{ sessionId: "sess:1", snapshotId: "snap:1", pinnedScope: null, stale: false }}
        messages={[{
          role: "assistant",
          text: "answer",
          claims: [{
            id: "claim:1",
            text: "有证据",
            classification: "grounded_interpretation",
            evidenceRefs: ["rel:a-b"],
            coverageCaveatRefs: [],
          }],
        }]}
        streaming={false}
        available
        onSend={() => {}}
        onCancel={() => {}}
        onNavigate={onNavigate}
      />,
    );

    const chip = screen.getByTitle("rel:a-b");
    expect(chip.tagName).toBe("BUTTON");
    fireEvent.click(chip);
    expect(onNavigate).toHaveBeenCalledWith({
      type: "focusRelation",
      relationKey: "rel:a-b",
      snapshotId: "snap:1",
    });
  });
});
