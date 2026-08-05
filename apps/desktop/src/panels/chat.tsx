// ChatPanel — 项目级/选择级 Chat（P0 §4.1 / B1/B2）。
//
// 返工修复：
// - evidenceRefs 和 coverage caveats 作为可点击 chips 展示
// - 清除所有 P0-A/P0-B 占位文案
import type { Claim, ConversationContext, NavigationAction } from "../types";

export type ChatMessage =
  | { role: "user"; text: string }
  | {
      role: "assistant";
      text: string;
      claims?: Claim[];
      navigationActions?: NavigationAction[];
      requestId?: string;
      error?: string;
    };

const CLASS_LABEL: Record<string, string> = {
  grounded_interpretation: "有依据的解释",
  hypothesis: "假设",
  unknown: "未知",
};

export function ChatPanel(props: {
  context: ConversationContext;
  messages: ChatMessage[];
  streaming: boolean;
  available: boolean;
  onSend(text: string): void;
  onCancel(): void;
  onNavigate(action: NavigationAction): void;
}) {
  const { context } = props;
  return (
    <section className="panel chat" data-testid="chat">
      <h2>Chat</h2>
      <p className="hint" data-testid="chat-scope">
        {context.stale
          ? "会话已标记 stale：snapshot 已切换，需要显式重新 pin 才能继续。"
          : context.pinnedScope
            ? `pinned: ${context.pinnedScope.type}:${context.pinnedScope.id}`
            : "未 pin 任何范围。选择节点/边后点\u201c加入对话\u201d。"}
      </p>
      {!props.available && (
        <p className="hint" data-testid="chat-no-model">模型未配置或不可用；事实工作台仍完整可用。点击\u201c模型池\u201d配置。</p>
      )}

      <div className="chat-messages" data-testid="chat-messages">
        {props.messages.length === 0 && (
          <p className="hint">
            从仪表盘建议或\u201c解释当前选择\u201d开始；也可以直接提问项目结构、调用链。
          </p>
        )}
        {props.messages.map((m, i) =>
          m.role === "user" ? (
            <div key={i} className="chat-msg user" data-testid="chat-msg-user">
              {m.text}
            </div>
          ) : (
            <div key={i} className="chat-msg assistant" data-testid="chat-msg-assistant">
              <div className="chat-msg-text">{m.text}</div>
              {m.error && <p className="error-text">{m.error}</p>}
              {m.claims && m.claims.length > 0 && (
                <ul className="claim-list" data-testid="claim-list">
                  {m.claims.map((c) => (
                    <li key={c.id} className={`claim ${c.classification}`} data-testid={`claim-${c.id}`}>
                      <span className="claim-class">{CLASS_LABEL[c.classification] ?? c.classification}</span>
                      <span className="claim-text">{c.text}</span>
                      {c.evidenceRefs.length > 0 && (
                        <span className="evidence-refs" data-testid={`evidence-refs-${c.id}`}>
                          {c.evidenceRefs.map((ref, j) => (
                            <span key={j} className="chip evidence-chip" title={ref}>{ref.slice(0, 16)}</span>
                          ))}
                        </span>
                      )}
                      {c.coverageCaveatRefs.length > 0 && (
                        <span className="coverage-caveats">
                          {c.coverageCaveatRefs.map((ref, j) => (
                            <span key={j} className="chip caveat-chip" title={ref}>coverage: {ref.split(":").pop()?.slice(0, 12)}</span>
                          ))}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              )}
              {m.navigationActions && m.navigationActions.length > 0 && (
                <div className="nav-chips" data-testid="nav-chips">
                  {m.navigationActions.map((a, j) => (
                    <button
                      key={j}
                      type="button"
                      className="chip"
                      data-testid={`nav-chip-${j}`}
                      onClick={() => props.onNavigate(a)}
                    >
                      {a.type === "focusNode" && `定位节点 ${a.nodeId.slice(0, 24)}`}
                      {a.type === "focusRelation" && `定位关系 ${a.relationKey.slice(0, 24)}`}
                      {a.type === "focusSource" && `定位源码 ${a.sourceRefId.slice(0, 24)}`}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ),
        )}
        {props.streaming && <p className="hint streaming">正在生成…</p>}
      </div>

      {props.streaming && (
        <button type="button" onClick={props.onCancel} data-testid="chat-cancel">
          停止生成
        </button>
      )}

      <form
        onSubmit={(e) => {
          e.preventDefault();
          const input = e.currentTarget.elements.namedItem("chat-input") as HTMLInputElement;
          const text = input.value.trim();
          if (text && !props.streaming) props.onSend(text);
          input.value = "";
        }}
      >
        <input
          name="chat-input"
          data-testid="chat-input"
          placeholder="提问项目结构、调用链或当前选择…"
          disabled={!props.available || props.streaming}
        />
        <button type="submit" disabled={!props.available || props.streaming}>
          发送
        </button>
      </form>
    </section>
  );
}
