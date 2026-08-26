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
  it("keeps the graph as the primary canvas by sharing one right rail between dashboard and inspector", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "snap:layout",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });

    render(<WorkbenchApp transport={transport} />);

    await screen.findByTestId("dashboard");
    expect(screen.getByTestId("graph-pane-test-double")).toBeTruthy();
    expect(screen.queryByTestId("inspector")).toBeNull();

    const firstTreeNode = document.querySelector<HTMLElement>('[data-testid^="tree-node-"]');
    expect(firstTreeNode).not.toBeNull();
    fireEvent.click(firstTreeNode!);

    await waitFor(() => expect(screen.getByTestId("inspector")).toBeTruthy());
    expect(screen.queryByTestId("dashboard-strip")).toBeNull();
    expect(screen.getByRole("button", { name: "解释当前选择" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "加入对话" })).toBeTruthy();
  });

  it("opens a compact start rail and a status bar without raw session ids", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "snap:layout",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });

    render(<WorkbenchApp transport={transport} />);

    expect(await screen.findByText("从这里开始")).toBeTruthy();
    expect(screen.queryByText("项目事实")).toBeNull();
    expect(screen.getByTestId("chat-model-select").closest(".wb-header")).toBeTruthy();
    expect(screen.queryByTestId("chat-model-row")).toBeNull();
    const status = await screen.findByTestId("status-bar");
    expect(status.textContent).not.toMatch(/sessionId|snapshotId/);
    expect(status.textContent).toMatch(/模块图|文件图|符号图/);
  });

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

  it("hides generic coverage:calls chips and shows the current graph selection", () => {
    render(
      <ChatPanel
        context={{ sessionId: "sess:1", snapshotId: "snap:1", pinnedScope: null, stale: false }}
        selectionLabel="聚合边 scripts → (unknown)"
        messages={[{
          role: "assistant",
          text: "这条边是归并结果",
          claims: [{
            id: "claim:1",
            text: "外部命令调用",
            classification: "hypothesis",
            evidenceRefs: [],
            coverageCaveatRefs: ["coverage:project:calls"],
          }],
        }]}
        streaming={false}
        available
        onSend={() => {}}
        onCancel={() => {}}
        onNavigate={() => {}}
      />,
    );
    expect(screen.getByTestId("chat-scope").textContent).toContain("scripts → (unknown)");
    expect(screen.queryByText(/coverage:/)).toBeNull();
  });

  it("shows working-set chips and can remove one", () => {
    const onRemove = vi.fn();
    render(
      <ChatPanel
        context={{ sessionId: "sess:1", snapshotId: "snap:1", pinnedScope: null, stale: false }}
        workingSet={[
          { id: "edge:a", kind: "edge", label: "scripts → (unknown)", block: "聚合边：scripts → (unknown)" },
          { id: "edge:b", kind: "edge", label: "scripts → (root)", block: "聚合边：scripts → (root)" },
        ]}
        currentWorkingId="edge:b"
        messages={[]}
        streaming={false}
        available
        onSend={() => {}}
        onCancel={() => {}}
        onNavigate={() => {}}
        onRemoveWorkingSetItem={onRemove}
      />,
    );
    expect(screen.getByTestId("working-set").textContent).toContain("scripts → (unknown)");
    expect(screen.getByTestId("working-set-chip-edge:b").className).toContain("current");
    fireEvent.click(screen.getByRole("button", { name: "从已讨论清单移除 scripts → (unknown)" }));
    expect(onRemove).toHaveBeenCalledWith("edge:a");
  });

  it("keeps discussed graph items across questions so a later prompt can compare them", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    class RecordingChatTransport extends FakeDesktopTransport {
      override async modelsList(): Promise<{ default: string; models: unknown[] }> {
        return { default: "mock", models: [{ id: "mock" }] };
      }
    }
    const transport = new RecordingChatTransport(json, {
      id: "snap:working-set",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);

    const nodes = await waitFor(() => {
      const found = document.querySelectorAll<HTMLElement>('[data-testid^="tree-node-"]');
      expect(found.length).toBeGreaterThan(1);
      return found;
    });
    const firstId = nodes[0].getAttribute("data-testid")!.slice("tree-node-".length);
    const secondId = nodes[1].getAttribute("data-testid")!.slice("tree-node-".length);

    fireEvent.click(nodes[0]);
    const input = await screen.findByTestId("chat-input");
    await waitFor(() => expect((input as HTMLInputElement).disabled).toBe(false));
    fireEvent.change(input, { target: { value: "这条是什么" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    await waitFor(() => expect(transport.lastChatMessages).toHaveLength(1));
    await screen.findByTestId("chat-msg-assistant");
    expect(screen.getByTestId(`working-set-chip-node:${firstId}`)).toBeTruthy();

    fireEvent.click(nodes[1]);
    await waitFor(() => expect((input as HTMLInputElement).disabled).toBe(false));
    fireEvent.change(input, { target: { value: "两条有什么关联？" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    await waitFor(() => expect(transport.lastChatMessages).toHaveLength(2));

    const second = transport.lastChatMessages[1];
    expect(second).toContain("两条有什么关联？");
    expect(second).toContain("[已讨论的图元素]");
    expect(second).toContain("[最近对话]");
    expect(second).toContain("这条是什么");
    expect(second).toContain("[回答口吻] 适中");
    expect(screen.getByTestId(`working-set-chip-node:${firstId}`)).toBeTruthy();
    expect(screen.getByTestId(`working-set-chip-node:${secondId}`)).toBeTruthy();
  });

  it("clears discussed items and chat when a new project session binds", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    class ProjectSwitchTransport extends FakeDesktopTransport {
      createdForSnapshotIds: string[] = [];
      override async modelsList(): Promise<{ default: string; models: unknown[] }> {
        return { default: "mock", models: [{ id: "mock" }] };
      }
      override async sessionCreate(snapshotId: string): Promise<string> {
        this.createdForSnapshotIds.push(snapshotId);
        return `sess:${this.createdForSnapshotIds.length}`;
      }
      override async analyzeStatus() {
        return { state: "Completed", jobId: "job-switch", publishedSnapshotId: "snap:project-b", error: null };
      }
      override async loadSnapshot(snapshotId: string) {
        if (snapshotId === "snap:project-b") return super.loadSnapshot("snap:project-a");
        return super.loadSnapshot(snapshotId);
      }
    }
    const transport = new ProjectSwitchTransport(json, {
      id: "snap:project-a",
      rootLabel: "project-a",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);

    const node = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('[data-testid^="tree-node-"]');
      expect(found).not.toBeNull();
      return found!;
    });
    fireEvent.click(node);
    const input = await screen.findByTestId("chat-input");
    await waitFor(() => expect((input as HTMLInputElement).disabled).toBe(false));
    fireEvent.change(input, { target: { value: "这条是什么" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByTestId("working-set");
    await screen.findByTestId("chat-msg-user");

    fireEvent.click(screen.getByTestId("analyze-btn"));
    await waitFor(() => {
      expect(screen.queryByTestId("working-set")).toBeNull();
      expect(screen.queryByTestId("chat-msg-user")).toBeNull();
    }, { timeout: 5000 });
    expect(await screen.findByTestId("dashboard-strip")).toBeTruthy();
  }, 8000);

  it("shows a guided tree with Chinese labels and a pinned entry", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "snap:guide-tree",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);
    const start = await screen.findByTestId("tree-start");
    expect(start.textContent).toMatch(/入口/);
    expect(start.textContent).toMatch(/main/);
    const tree = screen.getByTestId("tree");
    expect(tree.textContent).toMatch(/文件/);
    expect(tree.textContent).toMatch(/文件夹/);
    expect(tree.textContent).not.toMatch(/\bpackage\b/);
    expect(tree.querySelector(".tree-kind")?.textContent).not.toBe("symbol");
    expect(screen.queryByTestId("tree-node-package:Cargo.toml")).toBeNull();
    fireEvent.click(screen.getByTestId("tree-node-file:src/lib.rs"));
    await waitFor(() => expect(screen.getByTestId("inspector")).toBeTruthy());
    expect(screen.getByTestId("inspector-aggregate-facts").textContent).toMatch(/文件/);
  });

  it("surfaces the analyzer failure reason instead of failing silently", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    class FailingAnalyzeTransport extends FakeDesktopTransport {
      override async analyzeStatus() {
        return {
          state: "Failed",
          jobId: "job-boom",
          publishedSnapshotId: null,
          error: "analyze failed with signal: 6 (SIGABRT)",
        };
      }
    }
    const transport = new FailingAnalyzeTransport(json, {
      id: "snap:analyze-fail",
      rootLabel: "open-nwe",
      language: "python",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);
    await screen.findByTestId("dashboard");
    fireEvent.click(screen.getByTestId("analyze-btn"));

    const banner = await screen.findByTestId("analyze-error", undefined, { timeout: 6000 });
    expect(banner.textContent).toContain("分析失败");
    expect(banner.textContent).toContain("SIGABRT");
    // 失败不该假装成功：按钮回到可再次点击的状态
    await waitFor(() =>
      expect((screen.getByTestId("analyze-btn") as HTMLButtonElement).disabled).toBe(false),
    );
  }, 10000);

  it("shows the project name rather than the snapshot file name", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "shell-portable-smoke.snapshot",
      rootLabel: "open-nwe",
      language: "shell",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);
    const label = await screen.findByTestId("snapshot-label");
    expect(label.textContent).toContain("open-nwe");
    expect(label.textContent).not.toContain(".snapshot");
  });

  it("lets the tree and inspector rail collapse", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "snap:layout",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);
    await screen.findByTestId("dashboard");
    fireEvent.click(screen.getByTestId("tree-collapse"));
    expect(screen.getByTestId("tree").className).toContain("collapsed");
    fireEvent.click(screen.getByTestId("rail-collapse"));
    expect(screen.getByTestId("side-rail").className).toContain("collapsed");
  });

  it("opens settings from the header and lets the chat pane be dragged wider", async () => {
    const json = readFileSync(fixturePath, "utf8");
    const data = JSON.parse(json);
    const transport = new FakeDesktopTransport(json, {
      id: "snap:layout",
      rootLabel: "test",
      language: "rust",
      createdAt: data.generatedAt,
    });
    render(<WorkbenchApp transport={transport} />);
    await screen.findByTestId("dashboard");
    expect(screen.getByTestId("settings-toggle").textContent).toBe("设置");
    fireEvent.click(screen.getByTestId("settings-toggle"));
    expect(await screen.findByTestId("explain-style-switch")).toBeTruthy();
    fireEvent.click(screen.getByTestId("explain-style-plain"));
    fireEvent.click(screen.getByTestId("model-pool-close"));

    const shell = await screen.findByTestId("chat-shell");
    expect(shell.style.width).toBeTruthy();
    expect(screen.getByTestId("chat-resize").getAttribute("aria-label")).toContain("拖动");
  });
});
