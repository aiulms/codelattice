// @vitest-environment jsdom
// 项目挑选器 e2e（多语言卡 3）：体检 → 单候选直通 / 多候选挑选 / 零候选报错 /
// 取消回 idle / unsupported 折叠。语言与标签必须来自所选体检行，不是写死值。
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { WorkbenchApp } from "../App";
import { FakeDesktopTransport } from "../transport/fake-transport";
import type { WorkspaceInspection } from "../types";

vi.mock("../panels/graph-pane", () => ({
  GraphPane: () => <section data-testid="graph-pane-test-double" />,
}));

const here = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.resolve(
  here,
  "../../../../fixtures/webui-snapshots/rust-portable-smoke.snapshot.json",
);

afterEach(() => cleanup());

function multiCandidateInspection(): WorkspaceInspection {
  return {
    schemaVersion: "codelattice.workspaceInspection.v1",
    projects: [
      {
        name: "backend",
        relativePath: "backend",
        language: "rust",
        confidence: "certain",
        evidence: { kind: "manifest", file: "Cargo.toml" },
        sourceFileCount: 2,
        analyzable: true,
      },
    ],
    sourceOnlyAreas: [
      {
        relativePath: "scripts/tools",
        language: "shell",
        confidence: "medium",
        evidence: { kind: "extension-histogram", extension: ".sh", count: 2 },
        sourceFileCount: 2,
        analyzable: true,
      },
      {
        relativePath: "scripts/py",
        language: "python",
        confidence: "medium",
        evidence: { kind: "extension-histogram", extension: ".py", count: 3 },
        sourceFileCount: 3,
        analyzable: false,
        reason: "language-support-disabled-in-this-binary",
      },
    ],
    unsupportedAreas: [
      {
        relativePath: "legacy",
        language: "java",
        confidence: "medium",
        evidence: { kind: "extension-histogram", extension: ".java", count: 2 },
        sourceFileCount: 2,
        analyzable: false,
        reason: "language-not-supported",
        recognition: "known-unsupported",
      },
      {
        relativePath: "gen",
        confidence: "low",
        evidence: { kind: "extension-histogram", extension: ".xyzfoo", count: 2 },
        sourceFileCount: 2,
        analyzable: false,
        reason: "language-not-supported",
        recognition: "unrecognized",
      },
    ],
  };
}

/** 记录 analyze 参数 + 完成态的多候选 transport。 */
class PickingTransport extends FakeDesktopTransport {
  analyzeCalls: Array<[string, string]> = [];
  constructor() {
    super(readFileSync(fixturePath, "utf8"), {
      id: "snap:pick",
      rootLabel: "old-project",
      language: "shell",
      createdAt: "2026-08-24T00:00:00Z",
    });
  }
  override async inspect(): Promise<WorkspaceInspection> {
    return multiCandidateInspection();
  }
  override async analyze(root: string, language: string): Promise<{ jobId: string }> {
    this.analyzeCalls.push([root, language]);
    return { jobId: "job-pick" };
  }
  override async analyzeStatus(): Promise<{ state: string; jobId: string | null; publishedSnapshotId?: string | null; error?: string | null }> {
    return { state: "Completed", jobId: "job-pick", publishedSnapshotId: "snap:pick", error: null };
  }
}

async function openPicker(transport: FakeDesktopTransport) {
  render(<WorkbenchApp transport={transport} />);
  await screen.findByTestId("dashboard");
  fireEvent.click(screen.getByTestId("analyze-btn"));
  await screen.findByTestId("project-picker");
}

describe("project picker (workspace inspection)", () => {
  it("single analyzable row goes straight to analyze with that row's language", async () => {
    class SinglePythonTransport extends FakeDesktopTransport {
      analyzeCalls: Array<[string, string]> = [];
      override async inspect(): Promise<WorkspaceInspection> {
        return {
          schemaVersion: "codelattice.workspaceInspection.v1",
          projects: [],
          sourceOnlyAreas: [{
            relativePath: ".",
            language: "python",
            confidence: "medium",
            evidence: { kind: "extension-histogram", extension: ".py", count: 3 },
            sourceFileCount: 3,
            analyzable: true,
          }],
          unsupportedAreas: [],
        };
      }
      override async analyze(root: string, language: string): Promise<{ jobId: string }> {
        this.analyzeCalls.push([root, language]);
        return { jobId: "job-single" };
      }
    }
    const transport = new SinglePythonTransport(readFileSync(fixturePath, "utf8"), {
      id: "snap:single",
      rootLabel: "single",
      language: "python",
      createdAt: "2026-08-24T00:00:00Z",
    });
    render(<WorkbenchApp transport={transport} />);
    await screen.findByTestId("dashboard");

    fireEvent.click(screen.getByTestId("analyze-btn"));
    await waitFor(() => expect(transport.analyzeCalls.length).toBe(1));
    // 直通：不弹挑选器，analyze 收到的是体检行语言，不是写死的 rust
    expect(transport.analyzeCalls[0]).toEqual(["/fake/project", "python"]);
    expect(screen.queryByTestId("project-picker")).toBeNull();
  });

  it("multiple candidates open the picker; selecting a row drives root+language+labels", async () => {
    const transport = new PickingTransport();
    await openPicker(transport);

    // 灰显行在场不可点（未编 feature 的 python 区）
    expect(screen.getByTestId("picker-row-scripts/py-python").className ?? "").toMatch(/disabled|picker-row/);
    // 主列表行可点：backend（manifest）与 scripts/tools（直方图）
    fireEvent.click(screen.getByTestId("picker-row-backend-rust"));

    await waitFor(() => expect(transport.analyzeCalls.length).toBe(1));
    // root 按所选行拼接，language 来自该行
    expect(transport.analyzeCalls[0]).toEqual(["/fake/project/backend", "rust"]);
    // 挑选器卸载
    await waitFor(() => expect(screen.queryByTestId("project-picker")).toBeNull());
    // snapshotMeta 随所选行更新：rootLabel 是行的 name（backend），不是对话框根（project）
    const label = await screen.findByTestId("snapshot-label");
    await waitFor(() => expect(label.textContent).toBe("backend · rust"), { timeout: 6000 });
  });

  it("zero analyzable rows surface the failure banner with unsupported hint", async () => {
    class EmptyInspectionTransport extends FakeDesktopTransport {
      override async inspect(): Promise<WorkspaceInspection> {
        return {
          schemaVersion: "codelattice.workspaceInspection.v1",
          projects: [],
          sourceOnlyAreas: [],
          unsupportedAreas: multiCandidateInspection().unsupportedAreas,
        };
      }
      override async analyze(): Promise<{ jobId: string }> {
        throw new Error("analyze must not be called");
      }
    }
    const transport = new EmptyInspectionTransport(readFileSync(fixturePath, "utf8"), {
      id: "snap:empty",
      rootLabel: "empty",
      language: "rust",
      createdAt: "2026-08-24T00:00:00Z",
    });
    render(<WorkbenchApp transport={transport} />);
    await screen.findByTestId("dashboard");
    fireEvent.click(screen.getByTestId("analyze-btn"));

    const banner = await screen.findByTestId("analyze-error");
    expect(banner.textContent).toContain("未发现可分析项目");
    expect(banner.textContent).toContain("检测到 2 个暂不支持的语言区域");
    expect(screen.queryByTestId("project-picker")).toBeNull();
  });

  it("cancelling the picker returns to idle without error or analyze", async () => {
    const transport = new PickingTransport();
    await openPicker(transport);

    fireEvent.click(screen.getByTestId("picker-cancel"));
    await waitFor(() => expect(screen.queryByTestId("project-picker")).toBeNull());
    // 回 idle：按钮可再点、无错误横幅、analyze 从未被调
    expect((screen.getByTestId("analyze-btn") as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByTestId("analyze-error")).toBeNull();
    expect(transport.analyzeCalls).toEqual([]);
  });

  it("unsupported areas are collapsed by default and expand read-only", async () => {
    const transport = new PickingTransport();
    await openPicker(transport);

    // 折叠区容器在场，但内容行默认不可见（真实仓库 unrecognized 行极多）
    const box = screen.getByTestId("picker-unsupported");
    expect(box.textContent).toContain("暂不支持的区域");
    expect(screen.queryByText("legacy")).toBeNull();

    fireEvent.click(box.querySelector(".picker-unsupported-toggle")!);
    await waitFor(() => expect(screen.queryByText("legacy")).not.toBeNull());
    expect(box.textContent).toContain("java");
    // 展开后只读：unsupported 行没有可点按钮
    expect(box.querySelectorAll("li button").length).toBe(0);
  });
});
