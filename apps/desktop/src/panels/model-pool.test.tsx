// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ModelPoolPanel } from "./model-pool";
import { FakeDesktopTransport } from "../transport/fake-transport";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.resolve(
  here,
  "../../../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json",
);

class ModelRecordingTransport extends FakeDesktopTransport {
  added: Array<Record<string, unknown>> = [];
  secretWrites: Array<{ service: string; account: string; secret: string }> = [];

  override async modelsAdd(config: Record<string, unknown>): Promise<void> {
    this.added.push(config);
  }

  override async secretSet(service: string, account: string, secret: string): Promise<{ secretRef: string }> {
    this.secretWrites.push({ service, account, secret });
    return { secretRef: `keychain:${service}/${account}` };
  }
}

class ExistingModelTransport extends ModelRecordingTransport {
  override async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return {
      default: "remote-main",
      models: [{
        id: "remote-main",
        provider: "openai-compatible",
        base_url: "https://gateway.example/v1",
        model: "example-chat",
        api_key_ref: "keychain:codelattice/remote-main",
      }],
    };
  }
}

function transport() {
  const json = readFileSync(fixturePath, "utf8");
  return new ModelRecordingTransport(json, {
    id: "snap:model-pool",
    rootLabel: "test",
    language: "rust",
    createdAt: "2026-08-05T00:00:00Z",
  });
}

afterEach(cleanup);

describe("ModelPoolPanel", () => {
  it("makes OpenAI-compatible configuration explicit and stores its key before the model config", async () => {
    const tx = transport();
    render(<ModelPoolPanel transport={tx} open onClose={() => {}} />);

    fireEvent.click(await screen.findByRole("button", { name: "OpenAI 兼容" }));
    fireEvent.change(screen.getByLabelText("配置名称"), { target: { value: "deepseek-main" } });
    fireEvent.change(screen.getByLabelText("API Base URL"), { target: { value: "https://api.deepseek.com/v1" } });
    fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "test-key-not-real" } });
    fireEvent.change(screen.getByLabelText("模型 ID"), { target: { value: "deepseek-chat" } });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));

    await waitFor(() => expect(tx.added).toHaveLength(1));
    expect(tx.secretWrites).toEqual([{
      service: "codelattice",
      account: "deepseek-main",
      secret: "test-key-not-real",
    }]);
    expect(tx.added[0]).toEqual({
      id: "deepseek-main",
      provider: "openai-compatible",
      base_url: "https://api.deepseek.com/v1",
      model: "deepseek-chat",
      api_key_ref: "keychain:codelattice/deepseek-main",
    });
  });

  it("normalizes the Rust snake_case wire format before rendering configured models", async () => {
    const base = transport();
    const tx = new ExistingModelTransport(
      readFileSync(fixturePath, "utf8"),
      base.snapshots[0],
    );
    render(<ModelPoolPanel transport={tx} open onClose={() => {}} />);

    expect(await screen.findByText("https://gateway.example/v1")).toBeTruthy();
    expect(screen.getByText("已存入系统安全存储")).toBeTruthy();
  });
});
