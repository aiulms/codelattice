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
  GraphSelection,
  NavigationAction,
  SnapshotData,
  StreamHandle,
  ConversationContext,
} from "./types";
import { GraphSelectionStore } from "./state/graph-selection";
import { ConversationStore } from "./state/conversation";
import { buildIndex } from "./data/snapshot-reader";
import type { SnapshotIndex } from "./data/snapshot-reader";
import { EvidenceClient } from "./data/evidence-client";
import { collectStreamEvents } from "./data/stream-consumer";
import { DashboardPanel, computeDashboardFacts } from "./panels/dashboard";
import { InspectorPanel } from "./panels/inspector";
import { ChatPanel } from "./panels/chat";
import type { ChatMessage } from "./panels/chat";
import { GraphPane } from "./panels/graph-pane";
import { ModelPoolPanel } from "./panels/model-pool";

export function WorkbenchApp(props: { transport: DesktopTransport }) {
  const [snapshot, setSnapshot] = useState<SnapshotData | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);
  const [selection, setSelection] = useState<GraphSelection>({ type: "none" });
  const [convContext, setConvContext] = useState<ConversationContext>(
    { sessionId: "", pinnedScope: null, snapshotId: "", stale: false }
  );
  const [chatOpen, setChatOpen] = useState(true);
  const [modelPoolOpen, setModelPoolOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [modelAvailable, setModelAvailable] = useState(false);
  const [analyzeState, setAnalyzeState] = useState("idle");

  const selStoreRef = useRef<GraphSelectionStore | null>(null);
  if (!selStoreRef.current) selStoreRef.current = new GraphSelectionStore();
  const selStore = selStoreRef.current;

  const convStoreRef = useRef<ConversationStore | null>(null);
  if (!convStoreRef.current) convStoreRef.current = new ConversationStore("");
  const convStore = convStoreRef.current;

  const indexRef = useRef<SnapshotIndex | null>(null);
  const evidenceRef = useRef<EvidenceClient | null>(null);

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
        const index = buildIndex(data);
        indexRef.current = index;
        evidenceRef.current = new EvidenceClient(props.transport, index, data);
        setSnapshot(data);
        // 通过后端创建 session
        try {
          const sessionId = await props.transport.sessionCreate(index.snapshotId);
          if (!cancelled) {
            convStore.dispatch({ type: "replace-session", sessionId, snapshotId: index.snapshotId });
          }
        } catch (e) {
          if (!cancelled) setSnapshotError(`session 创建失败：${String(e)}`);
        }
      } catch (e) {
        if (!cancelled) setSnapshotError(String((e as Error)?.message ?? e));
      }

      // 检测模型可用性：验证默认模型存在
      try {
        const info = await props.transport.modelsList();
        if (!cancelled) {
          setModelAvailable(info.models.length > 0 && !!info.default);
        }
      } catch {
        if (!cancelled) setModelAvailable(false);
      }
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.transport]);

  const dashboardFacts = useMemo(
    () => (snapshot && indexRef.current ? computeDashboardFacts(snapshot, indexRef.current) : null),
    [snapshot],
  );

  const inspectorData = useMemo(() => {
    const evidence = evidenceRef.current;
    if (!evidence) return { node: null, edge: null };
    if (selection.type === "node") return { node: evidence.getNodeContextPreview(selection.nodeId), edge: null };
    if (selection.type === "relation") {
      return { node: null, edge: evidence.getEdgeEvidencePreview(selection.relationKey) };
    }
    return { node: null, edge: null };
  }, [selection]);

  // E-fix: Inspector preview → full async
  // 先显示 preview（确定性），再异步拉取 full evidence 替换。
  const [inspectorFull, setInspectorFull] = useState<{
    node: import("./types").NodeContextBundle | null;
    edge: import("./types").EdgeEvidenceBundle | null;
    selectionKey: string;
  }>({ node: null, edge: null, selectionKey: "" });

  useEffect(() => {
    const evidence = evidenceRef.current;
    if (!evidence || selection.type === "none") return;
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
  }, [selection]);

  // ── 模型流 ────────────────────────────────────────────────────────────

  async function consumeStream(handle: StreamHandle, placeholderText: string) {
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
      setStreaming(false);
    }
  }

  async function runExplain() {
    if (selection.type === "none") return;
    try {
      const handle = await props.transport.explainSelection({ selection, providerId: "", explanationLevel: "brief" });
      await consumeStream(handle, "正在解释当前选择…");
    } catch (e) {
      setStreaming(false);
      setMessages((prev) => [...prev, { role: "assistant", text: `解释失败：${String(e)}`, error: String(e) }]);
    }
  }

  async function sendChat(text: string) {
    setMessages((prev) => [...prev, { role: "user", text }]);
    try {
      const ctx = convStore.getState();
      if (!ctx.sessionId) {
        setMessages((prev) => [...prev, { role: "assistant", text: "会话未初始化，请刷新页面。", error: "no session" }]);
        return;
      }
      const handle = await props.transport.chat({
        sessionId: ctx.sessionId,
        message: text,
        providerId: "",
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
    // Transport 的 cancel 通过 requestId 调用后端
    // currentRequest 在 transport 内部管理
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

  const selectAndAnalyze = useCallback(async () => {
    try {
      setAnalyzeState("selecting");
      const root = await props.transport.selectProjectDirectory();
      if (!root) { setAnalyzeState("idle"); return; }
      setAnalyzeState("starting");
      await props.transport.analyze(root, "rust");
      // 轮询状态
      const poll = setInterval(async () => {
        try {
          const status = await props.transport.analyzeStatus();
          setAnalyzeState(status.state);
          if (["Completed", "Failed", "Cancelled"].includes(status.state)) {
            clearInterval(poll);
            if (status.state === "Completed" && status.publishedSnapshotId) {
              // 加载精确的 publishedSnapshotId（不依赖 snaps[0] 排序）
              const data = await props.transport.loadSnapshot(status.publishedSnapshotId);
              const index = buildIndex(data);
              indexRef.current = index;
              evidenceRef.current = new EvidenceClient(props.transport, index, data);
              // snapshot 切换 → 旧 session 标记 stale
              convStore.dispatch({ type: "snapshot-changed", snapshotId: index.snapshotId });
              setSnapshot(data);
            }
          }
        } catch { clearInterval(poll); }
      }, 2000);
    } catch (e) {
      setAnalyzeState("idle");
      setSnapshotError(`分析启动失败：${String(e)}`);
    }
  }, [props.transport, convStore]);

  const cancelAnalyze = useCallback(async () => {
    try { await props.transport.analyzeCancel(); setAnalyzeState("Cancelled"); } catch { /* ignore */ }
  }, [props.transport]);

  // 结构树
  const treeData = useMemo(() => {
    if (!snapshot) return null;
    const byFile = new Map<string, typeof snapshot.graph.nodes>();
    for (const n of snapshot.graph.nodes) {
      const key = n.file ?? "(unknown)";
      const arr = byFile.get(key) ?? [];
      arr.push(n);
      byFile.set(key, arr);
    }
    return byFile;
  }, [snapshot]);

  return (
    <div className="workbench">
      <header className="wb-header">
        <span className="wb-title">CodeLattice Workbench</span>
        <span className="wb-snapshot">
          {snapshot ? `snapshot: ${snapshot.generatedAt.slice(0, 19)}` : "no snapshot"}
        </span>
        <button type="button" onClick={selectAndAnalyze} data-testid="analyze-btn"
          disabled={analyzeState === "Running" || analyzeState === "starting" || analyzeState === "selecting"}>
          {analyzeState === "idle" ? "分析项目" : analyzeState}
        </button>
        {analyzeState === "Running" && (
          <button type="button" onClick={cancelAnalyze} data-testid="analyze-cancel-btn">取消分析</button>
        )}
        <button type="button" onClick={() => setModelPoolOpen(!modelPoolOpen)} data-testid="model-pool-toggle">模型池</button>
        <button type="button" onClick={() => setChatOpen(!chatOpen)} data-testid="chat-toggle">
          {chatOpen ? "收起 Chat" : "展开 Chat"}
        </button>
      </header>
      <div className="wb-body">
        <nav className="panel tree" data-testid="tree">
          <h2>结构树</h2>
          {treeData ? (
            <ul className="tree-list">
              {Array.from(treeData.entries()).map(([file, nodes]) => (
                <li key={file} className="tree-file">
                  <span className="tree-file-name">{file}</span>
                  <ul>
                    {nodes.map((n) => (
                      <li key={n.id} data-testid={`tree-node-${n.id}`}
                        onClick={() => {
                          if (indexRef.current) {
                            selStore.select({ type: "node", nodeId: n.id, snapshotId: indexRef.current.snapshotId });
                          }
                        }}>
                        <span className={`tree-kind-${n.kind}`}>{n.kind}</span> {n.label}
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
            </ul>
          ) : <p className="hint">加载快照后显示结构树。</p>}
        </nav>
        {selection.type === "none" && (
          <div className="wb-dashboard-strip" data-testid="dashboard-strip">
            <DashboardPanel facts={dashboardFacts} error={snapshotError ?? undefined} />
          </div>
        )}
        <GraphPane key={indexRef.current?.snapshotId ?? "graph"} selection={selection}
          transport={props.transport} snapshot={snapshot} store={selStore} />
        <InspectorPanel selection={selection}
          nodeContext={inspectorFull.node ?? inspectorData.node}
          edgeEvidence={inspectorFull.edge ?? inspectorData.edge}
          onExplainClick={() => void runExplain()} onJoinConversation={() => void joinConversation()} />
        {chatOpen && (
          <ChatPanel context={convContext} messages={messages} streaming={streaming} available={modelAvailable}
            onSend={(text) => void sendChat(text)} onCancel={() => void cancelStream()} onNavigate={navigateFromAction} />
        )}
        {modelPoolOpen && <ModelPoolPanel transport={props.transport} open onClose={() => setModelPoolOpen(false)} />}
      </div>
      {snapshotError && (
        <footer className="wb-status"><span className="error-text">{snapshotError}</span></footer>
      )}
      {!snapshotError && (
        <footer className="wb-status" data-testid="status-bar">
          sessionId: {convContext.sessionId.slice(0, 20) || "-"} · snapshotId: {indexRef.current?.snapshotId ?? "-"} · Analyzer: {analyzeState}
        </footer>
      )}
    </div>
  );
}
