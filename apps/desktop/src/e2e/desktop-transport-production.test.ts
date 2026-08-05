import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatRequest } from "../types";

const mocks = vi.hoisted(() => ({
  invocations: [] as Array<{ command: string; args: unknown }>,
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unsubscribe: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string, args?: unknown) => {
    mocks.invocations.push({ command, args });
    return undefined;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(name, callback);
    return mocks.unsubscribe;
  }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.open }));

import { TauriDesktopTransport } from "../transport/desktop-transport";

describe("TauriDesktopTransport production boundaries", () => {
  beforeEach(() => {
    mocks.invocations.length = 0;
    mocks.listeners.clear();
    mocks.unsubscribe.mockReset();
    mocks.open.mockReset();
  });

  it("uses the Tauri 2 dialog plugin instead of the fixture IPC command", async () => {
    mocks.open.mockResolvedValue("/tmp/selected-project");
    const transport = new TauriDesktopTransport();

    await expect(transport.selectProjectDirectory()).resolves.toBe("/tmp/selected-project");
    expect(mocks.open).toHaveBeenCalledWith({ directory: true, multiple: false });
    expect(mocks.invocations.some((entry) => entry.command === "workbench_select_directory")).toBe(false);
  });

  it("cancel terminates the local iterator and unsubscribes even without a backend terminal event", async () => {
    const transport = new TauriDesktopTransport();
    const handle = await transport.chat({} as ChatRequest);
    const next = handle.events[Symbol.asyncIterator]().next();

    await handle.cancel();
    const outcome = await Promise.race([
      next,
      new Promise<"timeout">((resolve) => setTimeout(() => resolve("timeout"), 50)),
    ]);

    expect(outcome).not.toBe("timeout");
    expect(outcome).toMatchObject({ done: true });
    expect(mocks.unsubscribe).toHaveBeenCalledTimes(1);
    expect(mocks.invocations.some((entry) => entry.command === "workbench_cancel")).toBe(true);
  });
});
