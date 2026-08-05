// Workbench App shell — 四区布局（P0 §4.1 布局约束）。
//
// 结构树 | 图谱 | Inspector | Chat；Chat 可折叠但不覆盖 Inspector。
// 状态全部来自显式 store：GraphSelectionStore / ConversationStore。
// P0-B1：解释当前选择 / Chat 流式 / 取消 / 模型池接入。
import { useEffect, useMemo, useRef, useState } from "react";
import type {
  DesktopTransport,
  GraphSelection,
  NavigationAction,
  SnapshotData,
  StreamHandle,
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
  const [chatOpen, setChatOpen] = useState(true);
  const [modelPoolOpen, setModelPoolOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const modelAvailable = true; // 模型池配置后置用；失败降级由 consume 内错误提示承担

  const selStoreRef = useRef<GraphSelectionStore | null>(null);
  if (!selStoreRef.current) selStoreRef.current = new GraphSelectionStore();
  const selStore = selStoreRef.current;

  const convStoreRef = useRef<ConversationStore | null>(null);
  if (!convStoreRef.current) convStoreRef.current = new ConversationStore("");
  const convStore = convStoreRef.current;

  const indexRef = useRef<SnapshotIndex | null>(null);
  const evidenceRef = useRef<EvidenceClient | null>(null);
  const activeStreamRef = useRef<StreamHandle | null>(null);

  useEffect(() => {
    const unsub = selStore.subscribe((s) => setSelection(s));
    return unsub;
  }, [selStore]);

  useEffect(() => {
    let cancelled = false;
    props.transport
      .listSnapshots()
      .then((snaps) => {
        if (cancelled || snaps.length === 0) return;
        return props.transport.loadSnapshot(snaps[0].id);
      })
      .then((data) => {
        if (cancelled || !data) return;
        const index = buildIndex(data);
        indexRef.current = index;
        evidenceRef.current = new EvidenceClient(props.transport, index, data);
        convStore.dispatch({ type: "snapshot-changed", snapshotId: index.snapshotId });
        setSnapshot(data);
      })
      .catch((e) => {
        if (!cancelled) setSnapshotError(String(e?.message ?? e));
      });
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

  // ── 模型流（P0-B1：解释当前选择 / Chat 共用 consume 管道）──────────────

  /** 消耗 StreamHandle：chunk 追加文本；complete 解析 claims/navigation；错误降级提示。 */
  async function consumeStream(handle: StreamHandle) {
    activeStreamRef.current = handle;
    setStreaming(true);
    try {
      const acc = await collectStreamEvents(handle.events, (progress) => {
        // 增量渲染：chunk 文本实时追加
        setMessages((prev) =>
          prev.map((m) =>
            m.role === "assistant" && m.requestId === handle.requestId
              ? { ...m, text: progress.text }
              : m,
          ),
        );
      });
      setMessages((prev) =>
        prev.map((m) =>
          m.role === "assistant" && m.requestId === handle.requestId
            ? {
                ...m,
                text: acc.text,
                claims: acc.claims,
                navigationActions: acc.navigationActions,
                error: acc.error,
              }
            : m,
        ),
      );
      if (acc.budgetLimit) {
        setMessages((prev) => [
          ...prev,
          { role: "assistant", text: `取证预算耗尽：${acc.budgetLimit}` },
        ]);
      }
    } catch (e) {
      setMessages((prev) =>
        prev.map((m) =>
          m.role === "assistant" && m.requestId === handle.requestId
            ? { ...m, error: String(e) }
            : m,
        ),
      );
    } finally {
      setStreaming(false);
      activeStreamRef.current = null;
    }
  }

  function startAssistant(scopeHint: string) {
    const msgId = `req:${Date.now().toString(36)}:${messages.length}`;
    setMessages((prev) => [
      ...prev,
      { role: "assistant", text: scopeHint ? `（${scopeHint}）` : "", requestId: msgId },
    ]);
    return msgId;
  }

  async function runExplain() {
    if (selection.type === "none") return;
    const msgId = startAssistant("正在解释当前选择…");
    try {
      const handle = await props.transport.explainSelection({
        selection,
        providerId: "",
        explanationLevel: "brief",
      });
      await consumeStream(handle);
    } catch (e) {
      setStreaming(false);
      setMessages((prev) =>
        prev.map((m) => (m.role === "assistant" && m.requestId === msgId ? { ...m, error: String(e) } : m)),
      );
    }
  }

  async function sendChat(text: string) {
    setMessages((prev) => [...prev, { role: "user", text }]);
    const msgId = startAssistant("");
    try {
      const handle = await props.transport.chat({
        sessionId: convStore.getState().sessionId,
        message: text,
        providerId: "",
      });
      await consumeStream(handle);
    } catch (e) {
      setStreaming(false);
      setMessages((prev) =>
        prev.map((m) => (m.role === "assistant" && m.requestId === msgId ? { ...m, error: String(e) } : m)),
      );
    }
  }

  async function cancelStream() {
    const handle = activeStreamRef.current;
    if (handle) {
      await handle.cancel().catch(() => {});
    }
  }

  /** evidence chip → 显式 NavigationRequest（验收 10/12）。 */
  function navigateFromAction(action: NavigationAction) {
    const index = indexRef.current;
    if (!index) return;
    const result = selStore.navigate(
      action,
      index.snapshotId,
      {
        node: (id) => index.nodeById.has(id),
        relation: (key) => index.relationByKey.has(key),
      },
      () => window.confirm("该导航目标属于另一个 snapshot，是否确认切换？"),
    );
    if (result.kind === "rejected") {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", text: `导航被拒绝：${result.reason}`, error: undefined },
      ]);
    }
  }

  return (
    <div className="workbench">
      <header className="wb-header">
        <span className="wb-title">CodeLattice Workbench</span>
        <span className="wb-snapshot">
          {snapshot ? `snapshot: ${snapshot.generatedAt.slice(0, 19)}` : "no snapshot"}
        </span>
        <button type="button" onClick={() => setModelPoolOpen(!modelPoolOpen)} data-testid="model-pool-toggle">
          模型池
        </button>
        <button type="button" onClick={() => setChatOpen(!chatOpen)} data-testid="chat-toggle">
          {chatOpen ? "收起 Chat" : "展开 Chat"}
        </button>
      </header>
      <div className="wb-body">
        <nav className="panel tree" data-testid="tree">
          <h2>结构树</h2>
          <p className="hint">P0-A 接入模块/文件树。</p>
        </nav>
        {selection.type === "none" && (
          <div className="wb-dashboard-strip" data-testid="dashboard-strip">
            <DashboardPanel facts={dashboardFacts} error={snapshotError ?? undefined} />
          </div>
        )}
        <GraphPane
          key={indexRef.current?.snapshotId ?? "graph"}
          selection={selection}
          transport={props.transport}
          snapshot={snapshot}
          store={selStore}
        />
        <InspectorPanel
          selection={selection}
          nodeContext={inspectorData.node}
          edgeEvidence={inspectorData.edge}
          onExplainClick={() => void runExplain()}
          onJoinConversation={() => {
            if (selection.type !== "none") {
              convStore.dispatch({
                type: "pin",
                scopeType: selection.type === "relation" ? "edge" : selection.type,
                id: selection.type === "node" ? selection.nodeId : selection.type === "relation" ? selection.relationKey : selection.chainId,
                snapshotId: selection.snapshotId,
              });
            }
          }}
        />
        {chatOpen && (
          <ChatPanel
            context={convStore.getState()}
            messages={messages}
            streaming={streaming}
            available={modelAvailable}
            onSend={(text) => void sendChat(text)}
            onCancel={() => void cancelStream()}
            onNavigate={navigateFromAction}
          />
        )}
        {modelPoolOpen && (
          <ModelPoolPanel
            transport={props.transport}
            open
            onClose={() => setModelPoolOpen(false)}
          />
        )}
      </div>
      {snapshotError && (
        <footer className="wb-status">
          <span className="error-text">snapshot 加载失败：{snapshotError}（静态降级路径见 P0-A）</span>
        </footer>
      )}
      {!snapshotError && (
        <footer className="wb-status" data-testid="status-bar">
          snapshotId: {indexRef.current?.snapshotId ?? "-"} · 静态分析边界 · Desktop Worker: -
        </footer>
      )}
    </div>
  );
}
