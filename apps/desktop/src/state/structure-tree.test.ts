// @vitest-environment node
import { describe, expect, it } from "vitest";
import type { SnapshotNode } from "../types";
import {
  buildStructureGuide,
  defaultOpenIds,
  kindLabelZh,
} from "./structure-tree";

function nodes(...list: SnapshotNode[]) {
  return { graph: { nodes: list } };
}

describe("kindLabelZh", () => {
  it("uses Chinese labels instead of raw kind strings", () => {
    expect(kindLabelZh("file")).toBe("文件");
    expect(kindLabelZh("folder")).toBe("文件夹");
    expect(kindLabelZh("symbol")).toBe("符号");
    expect(kindLabelZh("package")).toBe("包");
  });
});

describe("buildStructureGuide", () => {
  it("nests files under folders and hides package or command noise", () => {
    const guide = buildStructureGuide(nodes(
      { id: "pkg", label: "root", kind: "package", file: "" },
      { id: "shell:command:mkdir", label: "mkdir", kind: "mkdir", file: "" },
      { id: "shell:file:build.sh", label: "build.sh", kind: "build.sh", file: "build.sh" },
      { id: "shell:file:scripts/common.sh", label: "scripts/common.sh", kind: "file", file: "scripts/common.sh" },
      { id: "shell:symbol:build.sh:build_project", label: "build_project", kind: "symbol", file: "build.sh" },
      { id: "shell:symbol:scripts/common.sh:log_info", label: "log_info", kind: "symbol", file: "scripts/common.sh" },
    ));

    expect(guide.roots.map((row) => row.label)).toEqual(["scripts", "build.sh"]);
    const scripts = guide.roots.find((row) => row.label === "scripts");
    expect(scripts?.kind).toBe("folder");
    expect(scripts?.kindLabel).toBe("文件夹");
    expect(scripts?.children.map((row) => row.label)).toEqual(["common.sh"]);
    expect(scripts?.children[0].kindLabel).toBe("文件");
    expect(scripts?.children[0].jumpLevel).toBe("file");
    expect(scripts?.children[0].nodeId).toBe("file:scripts/common.sh");
    expect(scripts?.children[0].children.map((row) => row.label)).toEqual(["log_info"]);
    expect(scripts?.children[0].children[0].kindLabel).toBe("符号");
    expect(scripts?.children[0].children[0].jumpLevel).toBe("symbol");
  });

  it("pins snapshot entry points and marks the host file as an entry", () => {
    const guide = buildStructureGuide({
      insights: {
        entryPoints: [{ id: "symbol:main", name: "main", file: "src/main.rs" }],
      },
      graph: {
        nodes: [
          { id: "file:src/main.rs", label: "src/main.rs", kind: "file", file: "src/main.rs" },
          { id: "file:src/lib.rs", label: "src/lib.rs", kind: "file", file: "src/lib.rs" },
          { id: "symbol:main", label: "main", kind: "symbol", file: "src/main.rs" },
        ],
      },
    });

    expect(guide.startHere.map((row) => row.nodeId)).toContain("symbol:main");
    expect(guide.startHere[0]?.label).toBe("main");
    expect(guide.startHere[0]?.isEntry).toBe(true);
    const src = guide.roots.find((row) => row.label === "src");
    const mainFile = src?.children.find((row) => row.label === "main.rs");
    expect(mainFile?.isEntry).toBe(true);
  });

  it("infers well-known entry files when insights are empty", () => {
    const guide = buildStructureGuide({
      insights: { entryPoints: [] },
      ...nodes(
        { id: "shell:file:build.sh", label: "build.sh", kind: "file", file: "build.sh" },
        { id: "shell:file:scripts/test.sh", label: "test.sh", kind: "file", file: "scripts/test.sh" },
      ),
    });

    expect(guide.startHere.some((row) => row.label === "build.sh" && row.isEntry)).toBe(true);
    expect(guide.startHere.some((row) => row.label === "test.sh")).toBe(false);
  });

  it("hides script-entry symbols that only repeat the file name", () => {
    const guide = buildStructureGuide(nodes(
      { id: "shell:file:build.sh", label: "build.sh", kind: "file", file: "build.sh" },
      { id: "shell:symbol:build.sh:script-entry", label: "build.sh", kind: "symbol", file: "build.sh" },
      { id: "shell:symbol:build.sh:build_project", label: "build_project", kind: "symbol", file: "build.sh" },
    ));

    const file = guide.roots.find((row) => row.label === "build.sh");
    expect(file?.children.map((row) => row.label)).toEqual(["build_project"]);
  });

  it("opens folders by default and keeps files collapsed", () => {
    const guide = buildStructureGuide(nodes(
      { id: "file:src/lib.rs", label: "src/lib.rs", kind: "file", file: "src/lib.rs" },
      { id: "symbol:add", label: "add", kind: "symbol", file: "src/lib.rs" },
    ));
    const open = defaultOpenIds(guide);
    expect(open).toContain(guide.roots[0].id);
    expect(open).not.toContain(guide.roots[0].children[0].id);
  });
});
