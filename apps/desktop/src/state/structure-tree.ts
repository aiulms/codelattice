// 导览树：按文件夹/文件组织，钉住入口，隐藏分析器噪声。
// 模块边界仍只来自 snapshot；这里只做展示归类，不重算依赖。
import type { GraphLevel, SnapshotNode } from "../types";

export type TreeRowKind = "folder" | "file" | "symbol" | "package";

export type StructureTreeItem = {
  id: string;
  label: string;
  kind: TreeRowKind;
  kindLabel: string;
  path?: string;
  nodeId?: string;
  jumpLevel?: GraphLevel;
  isEntry: boolean;
  children: StructureTreeItem[];
  /** 文件/符号行的语言身份（快照节点自述，缺席即未知，前端不重算）。 */
  language?: string;
  /** 文件夹行的子孙语言去重——纯展示聚合，检查器/Chat 不得当事实陈述。 */
  languages?: string[];
};

export type StructureGuide = {
  startHere: StructureTreeItem[];
  roots: StructureTreeItem[];
};

const ENTRY_BASENAMES = new Set([
  "main.rs",
  "main.py",
  "main.c",
  "main.cc",
  "main.cpp",
  "main.ts",
  "main.js",
  "main.tsx",
  "index.ts",
  "index.js",
  "index.tsx",
  "build.sh",
  "lib.rs",
]);

export function kindLabelZh(kind: string): string {
  if (kind === "file") return "文件";
  if (kind === "folder") return "文件夹";
  if (kind === "symbol" || kind === "entry") return "符号";
  if (kind === "package") return "包";
  return "其他";
}

export function classifyNode(n: SnapshotNode): "file" | "symbol" | "package" | "noise" {
  if (n.kind === "package" || n.id.includes(":repo:") || n.id.startsWith("repo:") || n.id.startsWith("target:")) {
    return "package";
  }
  if (n.kind === "file" || /(^|:)file:/.test(n.id)) return "file";
  if (n.kind === "symbol" || n.kind === "entry" || /(^|:)symbol:/.test(n.id)) return "symbol";
  return "noise";
}

export function filePathOf(n: SnapshotNode): string | null {
  if (n.file && n.file !== "(unknown)") return n.file;
  if (classifyNode(n) === "file") {
    const path = n.label || n.file || "";
    return path || null;
  }
  return null;
}

export function isLikelyEntryFile(path: string): boolean {
  const base = path.split("/").pop() ?? path;
  return ENTRY_BASENAMES.has(base);
}

function fileBasename(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}

function isScriptEntryDuplicate(n: SnapshotNode, fileName: string): boolean {
  if (n.id.endsWith(":script-entry")) return true;
  return n.label === fileName || n.label === fileName.replace(/\.[^.]+$/, "");
}

function fileJumpId(path: string): string {
  return `file:${path}`;
}

type FileBucket = { fileNode?: SnapshotNode; symbols: SnapshotNode[] };

function collectFiles(nodes: SnapshotNode[]): Map<string, FileBucket> {
  const files = new Map<string, FileBucket>();
  for (const n of nodes) {
    const cls = classifyNode(n);
    if (cls === "noise" || cls === "package") continue;
    const path = filePathOf(n);
    if (!path) continue;
    const bucket = files.get(path) ?? { symbols: [] };
    if (cls === "file") bucket.fileNode = n;
    else bucket.symbols.push(n);
    files.set(path, bucket);
  }
  return files;
}

function insightEntries(
  nodes: SnapshotNode[],
  raw: unknown[] | undefined,
): StructureTreeItem[] {
  if (!raw) return [];
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const out: StructureTreeItem[] = [];
  for (const ep of raw) {
    if (typeof ep !== "object" || ep === null || !("id" in ep)) continue;
    const id = String((ep as { id: unknown }).id);
    if (!id) continue;
    const rec = ep as { name?: unknown; label?: unknown; file?: unknown };
    const node = byId.get(id);
    const label = String(rec.name ?? rec.label ?? node?.label ?? fileBasename(id));
    const path = node ? filePathOf(node) ?? undefined : typeof rec.file === "string" ? rec.file : undefined;
    const cls = node ? classifyNode(node) : "symbol";
    const kind: TreeRowKind = cls === "file" ? "file" : "symbol";
    out.push({
      id: `start:${id}`,
      label,
      kind,
      kindLabel: kindLabelZh(kind),
      path,
      nodeId: kind === "file" && path ? fileJumpId(path) : id,
      jumpLevel: kind === "file" ? "file" : "symbol",
      isEntry: true,
      children: [],
      language: node?.language,
    });
  }
  return out;
}

function collectStartHere(
  nodes: SnapshotNode[],
  files: Map<string, FileBucket>,
  insights?: { entryPoints?: unknown[] },
): StructureTreeItem[] {
  const startHere = insightEntries(nodes, insights?.entryPoints);
  const covered = new Set(startHere.map((row) => row.path).filter((p): p is string => !!p));

  if (startHere.length < 3) {
    const paths = [...files.keys()].sort();
    for (const path of paths) {
      if (startHere.length >= 3) break;
      if (!isLikelyEntryFile(path) || covered.has(path)) continue;
      covered.add(path);
      startHere.push({
        id: `start:${fileJumpId(path)}`,
        label: fileBasename(path),
        kind: "file",
        kindLabel: kindLabelZh("file"),
        path,
        nodeId: fileJumpId(path),
        jumpLevel: "file",
        isEntry: true,
        children: [],
      });
    }
  }
  return startHere.slice(0, 3);
}

type FolderAcc = { name: string; folders: Map<string, FolderAcc>; files: StructureTreeItem[] };

function nestByFolder(files: Map<string, FileBucket>, entryPaths: Set<string>): StructureTreeItem[] {
  const root: FolderAcc = { name: "", folders: new Map(), files: [] };
  for (const path of [...files.keys()].sort()) {
    const parts = path.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    let cur = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const name = parts[i];
      let next = cur.folders.get(name);
      if (!next) {
        next = { name, folders: new Map(), files: [] };
        cur.folders.set(name, next);
      }
      cur = next;
    }
    const fileName = parts[parts.length - 1] ?? path;
    const bucket = files.get(path)!;
    const symbols = bucket.symbols
      .filter((s) => !isScriptEntryDuplicate(s, fileName))
      .sort((a, b) => (a.line ?? 0) - (b.line ?? 0) || a.label.localeCompare(b.label))
      .map((s) => ({
        id: s.id,
        label: s.label,
        kind: "symbol" as const,
        kindLabel: kindLabelZh("symbol"),
        path,
        nodeId: s.id,
        jumpLevel: "symbol" as const,
        isEntry: s.kind === "entry" || s.label.toLowerCase() === "main",
        children: [],
        language: s.language,
      }));
    // 文件行语言：文件节点自述优先，缺省回落到桶内任一符号的语言（同一文件）
    const fileLanguage =
      bucket.fileNode?.language ?? bucket.symbols.find((s) => s.language)?.language;
    cur.files.push({
      id: bucket.fileNode?.id ?? fileJumpId(path),
      label: fileName,
      kind: "file",
      kindLabel: kindLabelZh("file"),
      path,
      nodeId: fileJumpId(path),
      jumpLevel: "file",
      isEntry: entryPaths.has(path) || isLikelyEntryFile(path),
      children: symbols,
      language: fileLanguage,
    });
  }

  /** 文件夹语言 = 子孙语言并集（展示聚合，非事实陈述）。 */
  function folderLanguages(children: StructureTreeItem[]): string[] | undefined {
    const set = new Set<string>();
    for (const child of children) {
      if (child.language) set.add(child.language);
      for (const lang of child.languages ?? []) set.add(lang);
    }
    return set.size > 0 ? [...set].sort() : undefined;
  }

  function folderToItem(folder: FolderAcc, prefix: string): StructureTreeItem {
    const path = prefix ? `${prefix}/${folder.name}` : folder.name;
    const children = [
      ...[...folder.folders.values()]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((child) => folderToItem(child, path)),
      ...folder.files,
    ];
    return {
      id: `folder:${path}`,
      label: folder.name,
      kind: "folder",
      kindLabel: kindLabelZh("folder"),
      path,
      isEntry: false,
      children,
      languages: folderLanguages(children),
    };
  }

  return [
    ...[...root.folders.values()]
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((folder) => folderToItem(folder, "")),
    ...root.files,
  ];
}

export function buildStructureGuide(data: {
  graph: { nodes: SnapshotNode[] };
  insights?: { entryPoints?: unknown[] };
}): StructureGuide {
  const files = collectFiles(data.graph.nodes);
  const startHere = collectStartHere(data.graph.nodes, files, data.insights);
  const entryPaths = new Set(startHere.map((row) => row.path).filter((p): p is string => !!p));
  return { startHere, roots: nestByFolder(files, entryPaths) };
}

export function defaultOpenIds(guide: StructureGuide): string[] {
  const ids: string[] = [];
  const walk = (items: StructureTreeItem[]) => {
    for (const item of items) {
      if (item.kind === "folder") {
        ids.push(item.id);
        walk(item.children);
      }
    }
  };
  walk(guide.roots);
  return ids;
}
