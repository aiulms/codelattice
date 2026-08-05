// Workbench App shell — 四区布局（P0 §4.1 布局约束）。
//
// 结构树 | 图谱 | Inspector | Chat；Chat 可折叠但不覆盖 Inspector。
// 状态全部来自显式 store：GraphSelectionStore / ConversationStore。
//
// 返工修复：
// - requestId 统一：App 不自造 msgId，直接用 transport 返回的 handle.requestId。
// - streaming 在所有终止路径后回到 false。
// - modelAvailable 动态判定（不再硬编码 true）。
// - 结构树从 snapshot 数据渲染（不再占位）。
// - Analyzer UI 入口（选择项目 / 启动 / 状态 / 取消）。
import { useEffect, useMemo, useRef, useState, useCallback } from "react";
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
  const [modelAvailable, setModelAvailable] = useState(false);
  // Analyzer 状态
  const [analyzeState, setAnalyzeState] = useState<string>("idle");
  const [analyzeRoot, setAnalyzeRoot] = useState<string>("");

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

    // 检测模型可用性
    props.transport
      .modelsList()
      .then((info) => {
        if (!cancelled) {
          setModelAvailable(info.models.length > 0);
        }
      })
      .catch(() => {
        if (!cancelled) setModelAvailable(false);
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

  // ── 模型流 ────────────────────────────────────────────────────────────

  /**
   * 消耗 StreamHandle：chunk 追加文本；complete 解析 claims/navigation；错误降级提示。
   * 返工修复：不再用 App 自造 msgId，直接用 handle.requestId 更新占位消息。
   */
  async function consumeStream(handle: StreamHandle, placeholderText: string) {
    activeStreamRef.current = handle;
    setStreaming(true);
    // 用 handle.requestId 创建占位消息
    setMessages((prev) => [
      ...prev,
      {
        role: "assistant",
        text: placeholderText,
        requestId: handle.requestId,
      },
    ]);
    try {
      const acc = await collectStreamEvents(handle.events, (progress) => {
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

  async function runExplain() {
    if (selection.type === "none") return;
    try {
      const handle = await props.transport.explainSelection({
        selection,
        providerId: "",
        explanationLevel: "brief",
      });
      await consumeStream(handle, "正在解释当前选择…");
    } catch (e) {
      setStreaming(false);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", text: `解释失败：${String(e)}`, error: String(e) },
      ]);
    }
  }

  async function sendChat(text: string) {
    setMessages((prev) => [...prev, { role: "user", text }]);
    try {
      const ctx = convStore.getState();
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
      setMessages((prev) => [
        ...prev,
        { role: "assistant", text: `Chat 失败：${String(e)}`, error: String(e) },
      ]);
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

  // ── Analyzer ──────────────────────────────────────────────────────────

  const selectAndAnalyze = useCallback(async () => {
    try {
      setAnalyzeState("selecting");
      const root = await props.transport.selectProjectDirectory();
      if (!root) {
        setAnalyzeState("idle");
        return;
      }
      setAnalyzeRoot(root);
      setAnalyzeState("starting");
      await props.transport.analyze(root, "rust");
      // 轮询状态
      const poll = setInterval(async () => {
        try {
          const status = await props.transport.analyzeStatus();
          setAnalyzeState(status.state);
          if (status.state === "Completed" || status.state === "Failed" || status.state === "Cancelled") {
            clearInterval(poll);
            if (status.state === "Completed") {
              // 刷新 snapshot 列表
              const snaps = await props.transport.listSnapshots();
              if (snaps.length > 0) {
                const data = await props.transport.loadSnapshot(snaps[0].id);
                const index = buildIndex(data);
                indexRef.current = index;
                evidenceRef.current = new EvidenceClient(props.transport, index, data);
                convStore.dispatch({ type: "snapshot-changed", snapshotId: index.snapshotId });
                setSnapshot(data);
              }
            }
          }
        } catch {
          clearInterval(poll);
        }
      }, 2000);
    } catch (e) {
      setAnalyzeState("idle");
      setSnapshotError(`分析启动失败：${String(e)}`);
    }
  }, [props.transport, convStore]);

  const cancelAnalyze = useCallback(async () => {
    try {
      await props.transport.analyzeCancel();
      setAnalyzeState("Cancelled");
    } catch {
      // ignore
    }
  }, [props.transport]);

  // 结构树：从 snapshot 真实渲染（不再占位）
  const treeData = useMemo(() => {
    if (!snapshot) return null;
    const nodes = snapshot.graph.nodes;
    // 按 file 分组
    const byFile = new Map<string, typeof nodes>();
    for (const n of nodes) {
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
          {analyzeState === "idle" ? "分析项目" : analyzeState === "Running" ? `分析中… ${analyzeRoot.slice(-20)}` : analyzeState}
        </button>
        {(analyzeState === "Running" || analyzeState === "starting") && (
          <button type="button" onClick={cancelAnalyze} data-testid="analyze-cancel-btn">取消分析</button>
        )}
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
          {treeData ? (
            <ul className="tree-list">
              {Array.from(treeData.entries()).map(([file, nodes]) => (
                <li key={file} className="tree-file">
                  <span className="tree-file-name">{file}</span>
                  <ul>
                    {nodes.map((n) => (
                      <li key={n.id} className="tree-node" data-testid={`tree-node-${n.id}`}
                        onClick={() => {
                          if (indexRef.current) {
                            selStore.select({ type: "node", nodeId: n.id, snapshotId: indexRef.current.snapshotId });
                          }
                        }}>
                        <span className={`tree-node-kind tree-kind-${n.kind}`}>{n.kind}</span>
                        {" "}{n.label}
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
            </ul>
          ) : (
            <p className="hint">加载快照后显示结构树。</p>
          )}
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
          <span className="error-text">snapshot 加载失败：{snapshotError}</span>
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
