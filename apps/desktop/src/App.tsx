// Workbench App shell（返工第二轮 A-fix / C-fix / E-fix）。
//
// 关键修复：
// - snapshot 加载后通过后端创建 session（不再前端自造 sessionId）
// - ConversationStore 接入 React subscription
// - "加入对话" 先调后端 sessionPin 再更新本地
// - Chat 携带 snapshotId + pinnedScope（必填）
// - Analyzer 完成后加载 publishedSnapshotId（不再依赖 snaps[0]）
// - modelAvailable 验证默认模型
// - stream cleanup 通过 requestId 匹配（不影响新请求）
import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import type {
  DesktopTransport,
  GraphLevel,
  GraphSelection,
  InspectionRow,
  NavigationAction,
  SnapshotData,
  StreamHandle,
  ConversationContext,
  WorkspaceInspection,
} from "./types";
import { GraphSelectionStore } from "./state/graph-selection";
import { ConversationStore } from "./state/conversation";
import {
  attachConversationMemory,
  conversationMemoryShouldReset,
  formatRecentTurns,
  formatWorkingSetPrompt,
  removeWorkingSetItem,
  selectionWorkingId,
  upsertWorkingSet,
  visibleUserChatText,
  workingItemFromSelection,
  type WorkingSetItem,
} from "./state/working-set";
import { aggregateSelectionFacts, buildIndex, formatSelectionForChat, selectionHeadline } from "./data/snapshot-reader";
import type { AggregateFacts, SnapshotIndex } from "./data/snapshot-reader";
import { applyTheme, readTheme, type ThemeName } from "./theme";
import {
  formatExplainStylePrompt,
  readExplainStyle,
  resolveExplainStyle,
  writeExplainStyle,
  type ExplainStyle,
} from "./state/explain-style";
import { clampChatWidth, readChatWidth, writeChatWidth } from "./state/chat-layout";
import { EvidenceClient } from "./data/evidence-client";
import { collectStreamEvents } from "./data/stream-consumer";
import { DashboardPanel, computeDashboardFacts } from "./panels/dashboard";
import { InspectorPanel } from "./panels/inspector";
import { ChatPanel } from "./panels/chat";
import type { ChatMessage } from "./panels/chat";
import { GraphPane } from "./panels/graph-pane";
import { ModelPoolPanel } from "./panels/model-pool";
import { StructureTreePanel } from "./panels/structure-tree";
import { ProjectPickerPanel } from "./panels/project-picker";
import { buildStructureGuide, type StructureTreeItem } from "./state/structure-tree";

/** 目录路径 → 项目名；标题栏不显示整条路径，也不显示快照文件名。 */
function projectLabelOf(root: string): string {
  return root.split(/[\\/]/).filter(Boolean).pop() ?? root;
}

/** 按根路径已有分隔符拼接子路径；webview 无 node:path，禁止 import "node:path"。 */
function joinUnderRoot(root: string, relativePath: string): string {
  if (relativePath === ".") return root;
  const sep = root.includes("\\") && !root.includes("/") ? "\\" : "/";
  return `${root.replace(/[\\/]+$/, "")}${sep}${relativePath}`;
}

/** 分析失败文案：讲清楚哪个项目、失败在哪一步。 */
function analyzeFailureText(root: string | null, reason?: string | null): string {
  const who = root ? `「${projectLabelOf(root)}」` : "该项目";
  const detail = reason && reason.trim() ? reason.trim() : "未知原因";
  return `分析失败：${who} 没能生成快照。${detail}`;
}

export function WorkbenchApp(props: { transport: DesktopTransport }) {
  const [snapshot, setSnapshot] = useState<SnapshotData | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);
  const [selection, setSelection] = useState<GraphSelection>({ type: "none" });
  const [convContext, setConvContext] = useState<ConversationContext>(
    { sessionId: "", pinnedScope: null, snapshotId: "", stale: false }
  );
  const [chatOpen, setChatOpen] = useState(true);
  const [modelPoolOpen, setModelPoolOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [modelAvailable, setModelAvailable] = useState(false);
  const [modelOptions, setModelOptions] = useState<{ id: string; model: string }[]>([]);
  const [defaultModelId, setDefaultModelId] = useState("");
  const [providerId, setProviderId] = useState("");
  const [analyzeState, setAnalyzeState] = useState("idle");
  const [graphLevel, setGraphLevel] = useState<GraphLevel>("symbol");
  const [theme, setTheme] = useState<ThemeName>(() => readTheme());
  const [explainStyle, setExplainStyle] = useState<ExplainStyle>(() => readExplainStyle());
  const [chatWidth, setChatWidth] = useState(() => readChatWidth());
  const chatWidthRef = useRef(chatWidth);
  const [snapshotMeta, setSnapshotMeta] = useState<{ language: string; rootLabel: string } | null>(null);
  const [treeCollapsed, setTreeCollapsed] = useState(false);
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [workingSet, setWorkingSet] = useState<WorkingSetItem[]>([]);
  const workingSetRef = useRef<WorkingSetItem[]>([]);
  // 挑选态（多语言卡 3）：体检发现 ≥2 个可分析候选时挂起，等用户选一行
  const [picker, setPicker] = useState<{ root: string; inspection: WorkspaceInspection } | null>(null);

  const selStoreRef = useRef<GraphSelectionStore | null>(null);
  if (!selStoreRef.current) selStoreRef.current = new GraphSelectionStore();
  const selStore = selStoreRef.current;

  const convStoreRef = useRef<ConversationStore | null>(null);
  if (!convStoreRef.current) convStoreRef.current = new ConversationStore("");
  const convStore = convStoreRef.current;

  const indexRef = useRef<SnapshotIndex | null>(null);
  const evidenceRef = useRef<EvidenceClient | null>(null);
  const activeStreamRef = useRef<StreamHandle | null>(null);
  const activeSessionIdRef = useRef("");
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);
  useEffect(() => {
    chatWidthRef.current = chatWidth;
  }, [chatWidth]);

  function changeExplainStyle(next: ExplainStyle) {
    setExplainStyle(next);
    writeExplainStyle(next);
  }

  const resizingRef = useRef(false);

  function onChatResizePointerDown(event: { preventDefault(): void; clientX: number }) {
    event.preventDefault();
    if (resizingRef.current) return;
    resizingRef.current = true;
    const startX = event.clientX;
    const startW = chatWidthRef.current;
    const onMove = (ev: { clientX: number }) => {
      setChatWidth(clampChatWidth(startW + (startX - ev.clientX)));
    };
    const onUp = () => {
      resizingRef.current = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("mouseup", onUp);
      writeChatWidth(chatWidthRef.current);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("mouseup", onUp);
  }

  const epochRef = useRef<{ sessionId: string; snapshotId: string } | null>(null);

  function resetConversationWorkspace() {
    workingSetRef.current = [];
    setWorkingSet([]);
    setMessages([]);
    selStore.dispatch({ type: "clear" });
    const handle = activeStreamRef.current;
    if (handle) {
      activeStreamRef.current = null;
      setStreaming(false);
      void handle.cancel();
    }
  }

  useEffect(() => {
    const next = { sessionId: convContext.sessionId, snapshotId: convContext.snapshotId };
    if (conversationMemoryShouldReset(epochRef.current, next)) {
      resetConversationWorkspace();
    }
    epochRef.current = next.sessionId ? next : null;
    // 工作集跟会话走；selStore 稳定，不列入 deps。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [convContext.sessionId, convContext.snapshotId]);

  useEffect(() => {
    if (!snapshot) return;
    const mods = snapshot.moduleGraph?.modules.length ?? 0;
    setGraphLevel(mods >= 2 ? "module" : "symbol");
  }, [snapshot]);

  // 订阅 stores → React state
  useEffect(() => {
    const unsub1 = selStore.subscribe((s) => setSelection(s));
    const unsub2 = convStore.subscribe((ctx) => setConvContext(ctx));
    return () => { unsub1(); unsub2(); };
  }, [selStore, convStore]);

  // 初始 snapshot 加载 + 创建 session + 检查模型可用性
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const snaps = await props.transport.listSnapshots();
        if (cancelled || snaps.length === 0) return;
        const data = await props.transport.loadSnapshot(snaps[0].id);
        if (cancelled || !data) return;
        const index = buildIndex(data, snaps[0].id);
        indexRef.current = index;
        evidenceRef.current = new EvidenceClient(props.transport, index, data);
        setSnapshotMeta({ language: snaps[0].language, rootLabel: snaps[0].rootLabel });
        setSnapshot(data);
        // 通过后端创建 session
        try {
          const sessionId = await props.transport.sessionCreate(index.snapshotId);
          if (cancelled) {
            void props.transport.sessionClose(sessionId).catch(() => {});
          } else {
            activeSessionIdRef.current = sessionId;
            convStore.dispatch({ type: "replace-session", sessionId, snapshotId: index.snapshotId });
          }
        } catch (e) {
          if (!cancelled) setSnapshotError(`session 创建失败：${String(e)}`);
        }
      } catch (e) {
        if (!cancelled) setSnapshotError(String((e as Error)?.message ?? e));
      }
    })();
    return () => {
      cancelled = true;
      const sessionId = activeSessionIdRef.current;
      activeSessionIdRef.current = "";
      if (sessionId) void props.transport.sessionClose(sessionId).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.transport]);

  // 模型池刷新：启动时与模型池面板关闭后拉取，供 Chat 模型选择器使用。
  const refreshModels = useCallback(async () => {
    try {
      const info = await props.transport.modelsList();
      // model 字段仅用于选择器展示，缺失不视为不可用
      const list = (info.models ?? [])
        .map((raw) => raw as { id?: unknown; model?: unknown })
        .filter((m): m is { id: string; model?: unknown } => typeof m.id === "string")
        .map((m) => ({ id: m.id, model: typeof m.model === "string" ? m.model : "" }));
      setModelOptions(list);
      setDefaultModelId(info.default ?? "");
      setModelAvailable(list.length > 0 && !!info.default);
      // 已选模型被删除时回退默认，避免后端 "model not found"
      setProviderId((current) => (current && list.some((m) => m.id === current) ? current : ""));
    } catch {
      setModelAvailable(false);
    }
  }, [props.transport]);

  useEffect(() => {
    if (!modelPoolOpen) void refreshModels();
  }, [modelPoolOpen, refreshModels]);

  const dashboardFacts = useMemo(
    () => (snapshot && indexRef.current ? computeDashboardFacts(snapshot, indexRef.current) : null),
    [snapshot],
  );

  const hasModuleGraph = (snapshot?.moduleGraph?.modules.length ?? 0) > 0;
  const aggregateFacts = useMemo(
    () => (snapshot ? aggregateSelectionFacts(snapshot, graphLevel, selection) : null),
    [snapshot, graphLevel, selection],
  );

  const inspectorData = useMemo(() => {
    const evidence = evidenceRef.current;
    if (!evidence || aggregateFacts) return { node: null, edge: null };
    if (selection.type === "node") return { node: evidence.getNodeContextPreview(selection.nodeId), edge: null };
    if (selection.type === "relation") {
      return { node: null, edge: evidence.getEdgeEvidencePreview(selection.relationKey) };
    }
    return { node: null, edge: null };
  }, [selection, aggregateFacts]);

  function changeGraphLevel(next: GraphLevel) {
    setGraphLevel(next);
    selStore.dispatch({ type: "clear" });
  }

  function jumpToSymbol(nodeId: string) {
    setGraphLevel("symbol");
    const index = indexRef.current;
    if (index) selStore.select({ type: "node", nodeId, snapshotId: index.snapshotId });
  }

  function jumpToTreeItem(item: StructureTreeItem) {
    // 不走 changeGraphLevel：那条路径会先清空选择，文件点击就会丢高亮。
    if (!item.nodeId || !item.jumpLevel) return;
    const index = indexRef.current;
    if (!index) return;
    setGraphLevel(item.jumpLevel);
    selStore.select({ type: "node", nodeId: item.nodeId, snapshotId: index.snapshotId });
  }

  // E-fix: Inspector preview → full async
  // 先显示 preview（确定性），再异步拉取 full evidence 替换。
  const [inspectorFull, setInspectorFull] = useState<{
    node: import("./types").NodeContextBundle | null;
    edge: import("./types").EdgeEvidenceBundle | null;
    selectionKey: string;
  }>({ node: null, edge: null, selectionKey: "" });

  useEffect(() => {
    const evidence = evidenceRef.current;
    if (!evidence || selection.type === "none" || selection.type === "multi" || aggregateFacts) return;
    const selKey = selection.type === "node" ? selection.nodeId
      : selection.type === "relation" ? selection.relationKey
      : selection.chainId;
    let cancelled = false;
    setInspectorFull({ node: null, edge: null, selectionKey: "" });

    if (selection.type === "node") {
      void evidence.getNodeContextFull(selection.snapshotId, selection.nodeId).then((full) => {
        if (!cancelled) setInspectorFull({ node: full, edge: null, selectionKey: selKey });
      }).catch(() => {});
    } else if (selection.type === "relation") {
      void evidence.getEdgeEvidenceFull(selection.snapshotId, selection.relationKey, selection.occurrenceKey).then((full) => {
        if (!cancelled) setInspectorFull({ node: null, edge: full, selectionKey: selKey });
      }).catch(() => {});
    }
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selection, graphLevel]);

  // ── 模型流 ────────────────────────────────────────────────────────────

  async function consumeStream(handle: StreamHandle, placeholderText: string) {
    activeStreamRef.current = handle;
    setStreaming(true);
    setMessages((prev) => [...prev, { role: "assistant", text: placeholderText, requestId: handle.requestId }]);
    try {
      const acc = await collectStreamEvents(handle.events, (progress) => {
        setMessages((prev) =>
          prev.map((m) => m.role === "assistant" && m.requestId === handle.requestId
            ? { ...m, text: progress.text } : m),
        );
      });
      setMessages((prev) =>
        prev.map((m) => m.role === "assistant" && m.requestId === handle.requestId
          ? { ...m, text: acc.text, claims: acc.claims, navigationActions: acc.navigationActions, error: acc.error }
          : m),
      );
    } catch (e) {
      setMessages((prev) =>
        prev.map((m) => m.role === "assistant" && m.requestId === handle.requestId
          ? { ...m, error: String(e) } : m),
      );
    } finally {
      if (activeStreamRef.current?.requestId === handle.requestId) {
        activeStreamRef.current = null;
        setStreaming(false);
      }
    }
  }

  function aggregatePrompt(facts: AggregateFacts): string {
    const rows = facts.rows.map(([k, v]) => `${k}：${v}`).join("\n");
    const lead = facts.kicker === "多选"
      ? `请分别解释这些已选中的图元素（已有边向上归并，不是新推断的依赖）：${facts.title}`
      : `请解释这条${facts.kicker}关系（已有边向上归并，不是新推断的依赖）：${facts.title}`;
    return [
      lead,
      rows,
      "约束：不要推断 snapshot 里没有的依赖；`(unknown)` 是没有文件路径的符号占位模块，不是未知第三方库；`external-command-invocation` 表示 Shell 调用了外部命令；最弱置信是这组底层边里最低的一条，不是平均值。",
    ].filter(Boolean).join("\n");
  }

  function rememberCurrentSelection(): WorkingSetItem[] {
    if (!snapshot || selection.type === "none") return workingSetRef.current;
    const block = formatSelectionForChat(snapshot, graphLevel, selection);
    const label = selectionHeadline(aggregateFacts, selection) ?? "当前选择";
    const item = workingItemFromSelection(selection, label, block ?? label);
    if (!item) return workingSetRef.current;
    const next = upsertWorkingSet(workingSetRef.current, item);
    workingSetRef.current = next;
    setWorkingSet(next);
    return next;
  }

  async function runExplain() {
    if (selection.type === "none") return;
    rememberCurrentSelection();
    if (aggregateFacts) {
      await sendChat(aggregatePrompt(aggregateFacts));
      return;
    }
    const title = selectionHeadline(null, selection) ?? "当前选择";
    const block = snapshot ? formatSelectionForChat(snapshot, graphLevel, selection) : null;
    await sendChat(
      [
        `请解释这个图元素：${title}`,
        block,
        "约束：不要推断 snapshot 里没有的依赖。",
      ].filter(Boolean).join("\n"),
    );
  }

  async function sendChat(text: string) {
    const discussed = rememberCurrentSelection();
    setMessages((prev) => [...prev, {
      role: "user",
      text: visibleUserChatText(
        text,
        aggregateFacts?.title ?? selectionHeadline(null, selection),
      ),
    }]);
    try {
      const ctx = convStore.getState();
      if (!ctx.sessionId) {
        setMessages((prev) => [...prev, { role: "assistant", text: "会话未初始化，请刷新页面。", error: "no session" }]);
        return;
      }
      if (ctx.stale) {
        setMessages((prev) => [...prev, {
          role: "assistant",
          text: "当前会话仍绑定旧 snapshot，请重新选择范围或等待新会话创建。",
          error: "stale session",
        }]);
        return;
      }
      const handle = await props.transport.chat({
        sessionId: ctx.sessionId,
        message: attachConversationMemory(text, {
          workingSetBlock: formatWorkingSetPrompt(discussed, selectionWorkingId(selection)),
          currentSelectionBlock: snapshot ? formatSelectionForChat(snapshot, graphLevel, selection) : null,
          recentTurns: formatRecentTurns(
            messages.map((m) => ({ role: m.role, text: m.text })),
            4,
          ),
          styleBlock: formatExplainStylePrompt(resolveExplainStyle(explainStyle, text)),
        }),
        providerId,
        snapshotId: ctx.snapshotId,
        pinnedScope: ctx.pinnedScope,
      });
      await consumeStream(handle, "");
    } catch (e) {
      setStreaming(false);
      setMessages((prev) => [...prev, { role: "assistant", text: `Chat 失败：${String(e)}`, error: String(e) }]);
    }
  }

  async function cancelStream() {
    const handle = activeStreamRef.current;
    if (!handle) return;
    await handle.cancel();
    if (activeStreamRef.current?.requestId === handle.requestId) {
      activeStreamRef.current = null;
      setStreaming(false);
    }
  }

  function navigateFromAction(action: NavigationAction) {
    const index = indexRef.current;
    if (!index) return;
    const result = selStore.navigate(action, index.snapshotId, {
      node: (id) => index.nodeById.has(id),
      relation: (key) => index.relationByKey.has(key),
    }, () => window.confirm("该导航目标属于另一个 snapshot，是否确认切换？"));
    if (result.kind === "rejected") {
      setMessages((prev) => [...prev, { role: "assistant", text: `导航被拒绝：${result.reason}` }]);
    }
  }

  // "加入对话"：先调后端 sessionPin，成功后再更新本地
  async function joinConversation() {
    if (selection.type === "none") return;
    if (aggregateFacts || selection.type === "multi") {
      if (aggregateFacts) await sendChat(aggregatePrompt(aggregateFacts));
      return;
    }
    const index = indexRef.current;
    if (!index) return;
    const scopeType = selection.type === "relation" ? "edge" : selection.type;
    const scopeId = selection.type === "node" ? selection.nodeId
      : selection.type === "relation" ? selection.relationKey
      : selection.chainId;
    const ctx = convStore.getState();
    if (!ctx.sessionId) return;
    try {
      await props.transport.sessionPin(ctx.sessionId, scopeType, scopeId, index.snapshotId);
      // 后端成功后再更新前端
      convStore.dispatch({ type: "pin", scopeType, id: scopeId, snapshotId: index.snapshotId });
    } catch (e) {
      setMessages((prev) => [...prev, { role: "assistant", text: `Pin 失败：${String(e)}`, error: String(e) }]);
    }
  }

  // ── Analyzer ──────────────────────────────────────────────────────────

  // 第二段：以体检行为准分析。语言与标签全部来自所选行（多语言卡 3），
  // 轮询/加载/会话切换逻辑保持原样。
  const startAnalyze = useCallback(async (root: string, row: InspectionRow) => {
    // analyzable 行在 CLI 契约里必有 language；缺席视为 inspect 契约破坏，
    // 显式报错，禁止回退 "rust"。提出局部常量：闭包内 TS 无法保持参数窄化。
    const language = row.language;
    if (!language) {
      setAnalyzeError(analyzeFailureText(root, `体检行 ${row.relativePath} 缺少 language 字段（inspect 契约破坏）`));
      setAnalyzeState("idle");
      return;
    }
    const analyzeRoot = joinUnderRoot(root, row.relativePath);
    // 标签来自所选行；"." 行没有子段时才回落对话框根目录名
    const rootLabel = row.name
      ?? (row.relativePath === "." ? projectLabelOf(root) : projectLabelOf(row.relativePath));
    try {
      setAnalyzeState("starting");
      await props.transport.analyze(analyzeRoot, language);
      // 轮询状态
      const poll = setInterval(async () => {
        try {
          const status = await props.transport.analyzeStatus();
          setAnalyzeState(status.state);
          if (["Completed", "Failed", "Cancelled"].includes(status.state)) {
            clearInterval(poll);
            // 失败必须说明原因：后端已经返回 error，之前被整段丢弃，
            // 用户只看到按钮闪一下、图还停在旧快照。
            if (status.state === "Failed") {
              setAnalyzeError(analyzeFailureText(analyzeRoot, status.error));
              setAnalyzeState("idle");
              return;
            }
            if (status.state === "Cancelled") {
              setAnalyzeState("idle");
              return;
            }
            if (status.state === "Completed" && !status.publishedSnapshotId) {
              setAnalyzeError(analyzeFailureText(analyzeRoot, "分析结束但没有产出可加载的快照"));
              setAnalyzeState("idle");
              return;
            }
            if (status.state === "Completed" && status.publishedSnapshotId) {
              // 加载精确的 publishedSnapshotId（不依赖 snaps[0] 排序）
              const data = await props.transport.loadSnapshot(status.publishedSnapshotId);
              const index = buildIndex(data, status.publishedSnapshotId);
              indexRef.current = index;
              evidenceRef.current = new EvidenceClient(props.transport, index, data);
              const newSessionId = await props.transport.sessionCreate(index.snapshotId);
              if (!mountedRef.current) {
                await props.transport.sessionClose(newSessionId).catch(() => {});
                return;
              }
              const oldSessionId = activeSessionIdRef.current;
              activeSessionIdRef.current = newSessionId;
              if (oldSessionId) {
                await props.transport.sessionClose(oldSessionId).catch(() => {});
              }
              convStore.dispatch({
                type: "replace-session",
                sessionId: newSessionId,
                snapshotId: index.snapshotId,
              });
              setSnapshotMeta({
                language,
                rootLabel,
              });
              setSnapshot(data);
              setAnalyzeState("idle");
            }
          }
        } catch (e) {
          // 轮询本身出错同样要说话，不能静默停表。
          clearInterval(poll);
          setAnalyzeError(analyzeFailureText(analyzeRoot, String(e)));
          setAnalyzeState("idle");
        }
      }, 2000);
    } catch (e) {
      setAnalyzeState("idle");
      setAnalyzeError(analyzeFailureText(analyzeRoot, String(e)));
    }
  }, [props.transport, convStore]);

  // 第一段：选目录 → 体检 → 单候选直通 / 零候选报错 / 多候选进挑选态。
  // 一切语言信息来自 inspect 信封，前端不自建检测（stop-line）。
  const selectAndAnalyze = useCallback(async () => {
    try {
      setAnalyzeError(null);
      setAnalyzeState("selecting");
      const root = await props.transport.selectProjectDirectory();
      if (!root) { setAnalyzeState("idle"); return; }

      let inspection: WorkspaceInspection;
      try {
        inspection = await props.transport.inspect(root);
      } catch (e) {
        setAnalyzeError(analyzeFailureText(root, `文件夹体检失败：${String(e)}`));
        setAnalyzeState("idle");
        return;
      }

      const candidates = [...inspection.projects, ...inspection.sourceOnlyAreas]
        .filter((row) => row.analyzable);
      if (candidates.length === 0) {
        const unsupportedCount = inspection.unsupportedAreas.length;
        const hint = unsupportedCount > 0 ? `检测到 ${unsupportedCount} 个暂不支持的语言区域。` : "";
        setAnalyzeError(analyzeFailureText(root, `未发现可分析项目。${hint}`.trim()));
        setAnalyzeState("idle");
        return;
      }
      if (candidates.length === 1) {
        // 单候选直通：保持既有单项目体验零回归
        await startAnalyze(root, candidates[0]);
        return;
      }
      // ≥2 候选：进入挑选态，等用户选择或取消
      setAnalyzeState("idle");
      setPicker({ root, inspection });
    } catch (e) {
      setAnalyzeState("idle");
      setAnalyzeError(analyzeFailureText(null, String(e)));
    }
  }, [props.transport, startAnalyze]);

  /** 挑选器选中一行：卸载挑选器后进现有 analyze 轮询。 */
  function pickProjectRow(row: InspectionRow) {
    const root = picker?.root;
    setPicker(null);
    if (!root) return;
    void startAnalyze(root, row);
  }

  const cancelAnalyze = useCallback(async () => {
    try { await props.transport.analyzeCancel(); setAnalyzeState("Cancelled"); } catch { /* ignore */ }
  }, [props.transport]);

  const treeGuide = useMemo(
    () => (snapshot ? buildStructureGuide(snapshot) : null),
    [snapshot],
  );
  const selectedTreeNodeId = selection.type === "node" ? selection.nodeId : null;

  return (
    <div className="workbench">
      <header className="wb-header">
        <span className="wb-title">CodeLattice Workbench</span>
        <span className="wb-snapshot" data-testid="snapshot-label">
          {snapshotMeta
            ? `${snapshotMeta.rootLabel} · ${snapshotMeta.language}`
            : snapshot
              ? snapshot.generatedAt.slice(0, 10)
              : "未加载快照"}
        </span>
        <button type="button" onClick={selectAndAnalyze} data-testid="analyze-btn"
          disabled={analyzeState === "Running" || analyzeState === "starting" || analyzeState === "selecting"}>
          {analyzeState === "idle" ? "分析项目" : analyzeState}
        </button>
        {analyzeState === "Running" && (
          <button type="button" onClick={cancelAnalyze} data-testid="analyze-cancel-btn">取消分析</button>
        )}
        <label className="wb-model-picker">
          <span>模型</span>
          <select
            id="chat-model-select"
            data-testid="chat-model-select"
            aria-label="当前模型"
            value={providerId}
            disabled={!modelAvailable}
            onChange={(event) => setProviderId(event.target.value)}
          >
            <option value="">
              {defaultModelId ? `默认（${defaultModelId}）` : "未配置"}
            </option>
            {modelOptions.map((m) => (
              <option key={m.id} value={m.id}>{m.id}{m.model ? ` · ${m.model}` : ""}</option>
            ))}
          </select>
        </label>
        <button type="button" onClick={() => setModelPoolOpen(!modelPoolOpen)} data-testid="settings-toggle">设置</button>
        <button type="button" onClick={() => setChatOpen(!chatOpen)} data-testid="chat-toggle">
          {chatOpen ? "收起 Chat" : "展开 Chat"}
        </button>
      </header>
      {analyzeError && (
        <div className="wb-banner" role="alert" data-testid="analyze-error">
          <span className="wb-banner-text">{analyzeError}</span>
          <button type="button" onClick={() => setAnalyzeError(null)} data-testid="analyze-error-dismiss">
            知道了
          </button>
        </div>
      )}
      <div className="wb-body">
        {picker && (
          <ProjectPickerPanel
            inspection={picker.inspection}
            onSelect={pickProjectRow}
            onCancel={() => setPicker(null)}
          />
        )}
        <nav className={`panel tree${treeCollapsed ? " collapsed" : ""}`} data-testid="tree">
          <div className="panel-head">
            {!treeCollapsed && <h2>结构树</h2>}
            <button
              type="button"
              className="panel-collapse"
              data-testid="tree-collapse"
              aria-label={treeCollapsed ? "展开结构树" : "收起结构树"}
              onClick={() => setTreeCollapsed((v) => !v)}
            >
              {treeCollapsed ? "›" : "‹"}
            </button>
          </div>
          {treeCollapsed ? <span className="collapsed-label">结构树</span> : (
            <StructureTreePanel
              guide={treeGuide}
              selectedNodeId={selectedTreeNodeId}
              onJump={jumpToTreeItem}
            />
          )}
        </nav>
        <GraphPane
          key={indexRef.current?.snapshotId ?? "graph"}
          selection={selection}
          transport={props.transport}
          snapshot={snapshot}
          snapshotId={indexRef.current?.snapshotId}
          store={selStore}
          level={graphLevel}
          onLevelChange={changeGraphLevel}
          hasModuleGraph={hasModuleGraph}
        />
        <div className={`wb-rail${railCollapsed ? " collapsed" : ""}`} data-testid="side-rail">
          <button
            type="button"
            className="panel-collapse rail-toggle"
            data-testid="rail-collapse"
            aria-label={railCollapsed ? "展开检查器" : "收起检查器"}
            onClick={() => setRailCollapsed((v) => !v)}
          >
            {railCollapsed ? "‹" : "›"}
          </button>
          {railCollapsed
            ? <span className="collapsed-label">{selection.type === "none" ? "开始" : "检查器"}</span>
            : selection.type === "none" ? (
            <div className="wb-dashboard-strip" data-testid="dashboard-strip">
              <DashboardPanel
                facts={dashboardFacts}
                error={snapshotError ?? undefined}
                hasModuleGraph={hasModuleGraph}
                onJumpLevel={changeGraphLevel}
                onJumpNode={jumpToSymbol}
              />
            </div>
          ) : (
            <InspectorPanel
              selection={selection}
              nodeContext={inspectorFull.node ?? inspectorData.node}
              edgeEvidence={inspectorFull.edge ?? inspectorData.edge}
              aggregateFacts={aggregateFacts}
              onExplainClick={() => void runExplain()}
              onJoinConversation={() => void joinConversation()}
            />
          )}
        </div>
        {chatOpen && (
          <div className="chat-shell" data-testid="chat-shell" style={{ width: chatWidth }}>
            <button
              type="button"
              className="chat-resize"
              data-testid="chat-resize"
              aria-label="拖动调整对话窗宽度"
              onPointerDown={onChatResizePointerDown}
              onMouseDown={onChatResizePointerDown}
            />
            <ChatPanel
              context={convContext}
              selectionLabel={selectionHeadline(aggregateFacts, selection)}
              workingSet={workingSet}
              currentWorkingId={selectionWorkingId(selection)}
              messages={messages}
              streaming={streaming}
              available={modelAvailable}
              onSend={(text) => void sendChat(text)}
              onCancel={() => void cancelStream()}
              onNavigate={navigateFromAction}
              onRemoveWorkingSetItem={(id) => {
                const next = removeWorkingSetItem(workingSetRef.current, id);
                workingSetRef.current = next;
                setWorkingSet(next);
              }}
            />
          </div>
        )}
        {modelPoolOpen && (
          <ModelPoolPanel
            transport={props.transport}
            open
            onClose={() => setModelPoolOpen(false)}
            theme={theme}
            onThemeChange={setTheme}
            explainStyle={explainStyle}
            onExplainStyleChange={changeExplainStyle}
          />
        )}
      </div>
      {snapshotError && (
        <footer className="wb-status"><span className="error-text">{snapshotError}</span></footer>
      )}
      {!snapshotError && (
        <footer className="wb-status" data-testid="status-bar">
          {snapshotMeta?.language ?? "—"}
          {" · "}
          {snapshot
            ? `${snapshot.graph.summary.nodeCount} 节点 · ${snapshot.graph.summary.edgeCount} 边`
            : "无快照"}
          {" · "}
          {graphLevel === "module" ? "模块图" : graphLevel === "file" ? "文件图" : "符号图"}
          {" · "}
          {analyzeState === "idle" ? "就绪" : analyzeState}
        </footer>
      )}
    </div>
  );
}
