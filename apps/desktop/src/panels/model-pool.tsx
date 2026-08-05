// ModelPoolPanel — 本地模型设置工作台。
//
// 后端只提供 Ollama / OpenAI-compatible 两种确定性契约；这里负责把它们呈现为
// “供应商 → 连接 → 模型”的可理解流程。Key 先写系统安全存储，models.json 只收 ref。
import { useEffect, useMemo, useState } from "react";
import type { DesktopTransport } from "../types";

export type ModelInfo = {
  id: string;
  provider: ProviderKind;
  baseUrl: string;
  model: string;
  apiKeyRef?: string | null;
};

type ProviderKind = "ollama" | "openai-compatible";
type Draft = {
  id: string;
  provider: ProviderKind;
  baseUrl: string;
  model: string;
  apiKey: string;
};

type ModelWireInfo = {
  id?: unknown;
  provider?: unknown;
  base_url?: unknown;
  baseUrl?: unknown;
  model?: unknown;
  api_key_ref?: unknown;
  apiKeyRef?: unknown;
};

const PROVIDERS: Record<ProviderKind, {
  title: string;
  short: string;
  description: string;
  defaultBaseUrl: string;
  modelPlaceholder: string;
}> = {
  "openai-compatible": {
    title: "OpenAI 兼容",
    short: "OA",
    description: "适用于 OpenAI、DeepSeek 及其他兼容 Chat Completions 的服务。",
    defaultBaseUrl: "https://api.openai.com/v1",
    modelPlaceholder: "例如 deepseek-chat / gpt-4.1-mini",
  },
  ollama: {
    title: "Ollama 本地",
    short: "OL",
    description: "连接本机 Ollama，不需要把代码证据发送到远程服务。",
    defaultBaseUrl: "http://127.0.0.1:11434/v1",
    modelPlaceholder: "例如 qwen3:14b",
  },
};

function newDraft(provider: ProviderKind): Draft {
  return {
    id: "",
    provider,
    baseUrl: PROVIDERS[provider].defaultBaseUrl,
    model: "",
    apiKey: "",
  };
}

function normalizeModel(raw: unknown): ModelInfo | null {
  const wire = (raw ?? {}) as ModelWireInfo;
  if (typeof wire.id !== "string" || typeof wire.model !== "string") return null;
  if (wire.provider !== "ollama" && wire.provider !== "openai-compatible") return null;
  const baseUrl = wire.base_url ?? wire.baseUrl;
  const apiKeyRef = wire.api_key_ref ?? wire.apiKeyRef;
  return {
    id: wire.id,
    provider: wire.provider,
    baseUrl: typeof baseUrl === "string" ? baseUrl : "",
    model: wire.model,
    apiKeyRef: typeof apiKeyRef === "string" ? apiKeyRef : null,
  };
}

export function ModelPoolPanel(props: {
  transport: DesktopTransport;
  open: boolean;
  onClose(): void;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [defaultId, setDefaultId] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(() => newDraft("openai-compatible"));
  const [status, setStatus] = useState<string>("");
  const [busy, setBusy] = useState<string | null>(null);

  const selected = useMemo(
    () => models.find((model) => model.id === selectedId) ?? null,
    [models, selectedId],
  );

  async function refresh(preferredId?: string) {
    try {
      const list = await props.transport.modelsList();
      const nextModels = (list.models ?? []).map(normalizeModel).filter((model): model is ModelInfo => model !== null);
      const nextDefault = list.default || null;
      setModels(nextModels);
      setDefaultId(nextDefault);
      setSelectedId((current) => {
        const wanted = preferredId ?? current;
        if (wanted && nextModels.some((model) => model.id === wanted)) return wanted;
        return nextModels.find((model) => model.id === nextDefault)?.id ?? nextModels[0]?.id ?? null;
      });
    } catch (error) {
      setStatus(`读取模型配置失败：${String(error)}`);
    }
  }

  useEffect(() => {
    if (!props.open) return;
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.open]);

  if (!props.open) return null;

  function beginAdd(provider: ProviderKind) {
    setSelectedId(null);
    setDraft(newDraft(provider));
    setStatus("");
  }

  async function addModel() {
    const id = draft.id.trim();
    const baseUrl = draft.baseUrl.trim().replace(/\/$/, "");
    const model = draft.model.trim();
    if (!id || !baseUrl || !model) {
      setStatus("请填写配置名称、API Base URL 和模型 ID。");
      return;
    }

    setBusy("saving");
    let apiKeyRef: string | null = null;
    try {
      if (draft.provider === "openai-compatible" && draft.apiKey.trim()) {
        const secret = await props.transport.secretSet("codelattice", id, draft.apiKey.trim());
        apiKeyRef = secret.secretRef;
      }
      await props.transport.modelsAdd({
        id,
        provider: draft.provider,
        base_url: baseUrl,
        model,
        api_key_ref: apiKeyRef,
      });
      setStatus(`已保存 ${id}。可以继续测试连接或设为默认。`);
      setDraft((current) => ({ ...current, apiKey: "" }));
      await refresh(id);
    } catch (error) {
      // 配置落盘失败时删除本轮刚写入的 secret，避免孤儿 Keychain 条目。
      if (apiKeyRef) await props.transport.secretDelete(apiKeyRef).catch(() => {});
      setStatus(`保存失败：${String(error)}`);
    } finally {
      setBusy(null);
    }
  }

  async function removeModel(id: string) {
    setBusy(`remove:${id}`);
    try {
      await props.transport.modelsRemove(id);
      setStatus(`已删除 ${id}，关联密钥也已清理。`);
      setSelectedId(null);
      await refresh();
    } catch (error) {
      setStatus(`删除失败：${String(error)}`);
    } finally {
      setBusy(null);
    }
  }

  async function makeDefault(id: string) {
    setBusy(`default:${id}`);
    try {
      await props.transport.modelsSetDefault(id);
      setDefaultId(id);
      setStatus(`${id} 已设为默认模型。`);
    } catch (error) {
      setStatus(`设为默认失败：${String(error)}`);
    } finally {
      setBusy(null);
    }
  }

  async function testModel(id: string) {
    setBusy(`test:${id}`);
    try {
      const result = await props.transport.modelsTest(id);
      setStatus(result.ok ? `${id} 连接成功。` : `${id} 连接失败：${result.detail}`);
    } catch (error) {
      setStatus(`连接测试失败：${String(error)}`);
    } finally {
      setBusy(null);
    }
  }

  const meta = PROVIDERS[draft.provider];

  return (
    <div
      className="model-settings-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) props.onClose();
      }}
    >
      <section
        className="model-settings"
        data-testid="model-pool"
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-settings-title"
      >
        <header className="model-settings-header">
          <div>
            <span className="settings-kicker">UNDERSTANDING LAYER</span>
            <h2 id="model-settings-title">模型设置</h2>
            <p>配置用于链路翻译和项目对话的模型；事实图谱不依赖任何模型。</p>
          </div>
          <button
            type="button"
            className="icon-button"
            onClick={props.onClose}
            data-testid="model-pool-close"
            aria-label="关闭模型设置"
          >
            ×
          </button>
        </header>

        <div className="model-settings-body">
          <aside className="provider-rail" aria-label="模型供应商">
            <div className="rail-section">
              <span className="rail-label">添加连接</span>
              {(Object.keys(PROVIDERS) as ProviderKind[]).map((provider) => {
                const providerMeta = PROVIDERS[provider];
                const active = !selected && draft.provider === provider;
                return (
                  <button
                    type="button"
                    key={provider}
                    className={`provider-entry ${active ? "active" : ""}`}
                    onClick={() => beginAdd(provider)}
                    aria-label={providerMeta.title}
                  >
                    <span className={`provider-mark ${provider === "ollama" ? "local" : "remote"}`}>
                      {providerMeta.short}
                    </span>
                    <span>
                      <strong>{providerMeta.title}</strong>
                      <small>{provider === "ollama" ? "localhost" : "custom API"}</small>
                    </span>
                    <span className="provider-plus">＋</span>
                  </button>
                );
              })}
            </div>

            <div className="rail-section configured-models">
              <span className="rail-label">已配置 · {models.length}</span>
              {models.map((model) => (
                <button
                  type="button"
                  key={model.id}
                  className={`provider-entry model-entry ${selectedId === model.id ? "active" : ""}`}
                  onClick={() => setSelectedId(model.id)}
                >
                  <span className={`provider-mark ${model.provider === "ollama" ? "local" : "remote"}`}>
                    {PROVIDERS[model.provider].short}
                  </span>
                  <span>
                    <strong>{model.id}</strong>
                    <small>{model.model}</small>
                  </span>
                  <span className={`status-dot ${model.id === defaultId ? "default" : ""}`} />
                </button>
              ))}
              {models.length === 0 && (
                <p className="rail-empty">还没有模型连接，从上方选择一种兼容方式开始。</p>
              )}
            </div>
          </aside>

          <main className="model-settings-content">
            {selected ? (
              <ModelDetail
                model={selected}
                isDefault={selected.id === defaultId}
                busy={busy}
                onTest={() => void testModel(selected.id)}
                onDefault={() => void makeDefault(selected.id)}
                onRemove={() => void removeModel(selected.id)}
              />
            ) : (
              <div className="connection-editor" data-testid="model-form">
                <div className="connection-heading">
                  <span className={`provider-mark large ${draft.provider === "ollama" ? "local" : "remote"}`}>
                    {meta.short}
                  </span>
                  <div>
                    <span className="settings-kicker">NEW CONNECTION</span>
                    <h3>添加 {meta.title}连接</h3>
                    <p>{meta.description}</p>
                  </div>
                </div>

                <div className="form-card">
                  <label className="field-row">
                    <span>配置名称</span>
                    <input
                      aria-label="配置名称"
                      value={draft.id}
                      placeholder={draft.provider === "ollama" ? "ollama-qwen" : "deepseek-main"}
                      autoComplete="off"
                      onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                    />
                    <small>本机唯一名称，用于 Chat 中选择模型。</small>
                  </label>

                  <label className="field-row">
                    <span>API Base URL</span>
                    <input
                      aria-label="API Base URL"
                      value={draft.baseUrl}
                      placeholder={meta.defaultBaseUrl}
                      inputMode="url"
                      onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
                    />
                    <small>填写到 `/v1`；应用会调用兼容的 Chat Completions 接口。</small>
                  </label>

                  {draft.provider === "openai-compatible" && (
                    <label className="field-row">
                      <span>API Key</span>
                      <input
                        aria-label="API Key"
                        type="password"
                        value={draft.apiKey}
                        placeholder="仅写入 macOS Keychain"
                        autoComplete="new-password"
                        onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })}
                      />
                      <small>不会进入 models.json、snapshot、日志或模型提示词。</small>
                    </label>
                  )}

                  <label className="field-row">
                    <span>模型 ID</span>
                    <input
                      aria-label="模型 ID"
                      value={draft.model}
                      placeholder={meta.modelPlaceholder}
                      autoComplete="off"
                      onChange={(event) => setDraft({ ...draft, model: event.target.value })}
                    />
                    <small>必须与供应商 API 接受的 model 字段完全一致。</small>
                  </label>
                </div>

                <div className="settings-actions">
                  <span className="security-note"><b>LOCAL SECRET</b> Key 只在本机解引用。</span>
                  <button
                    type="button"
                    className="primary-button"
                    onClick={() => void addModel()}
                    disabled={busy === "saving"}
                    data-testid="model-add"
                  >
                    {busy === "saving" ? "正在保存…" : "保存配置"}
                  </button>
                </div>
              </div>
            )}
          </main>
        </div>

        <footer className="model-settings-footer">
          <span className="privacy-pulse" />
          远程模型只接收当前问题所需的结构化证据；本地路径默认脱敏。
          {status && <strong role="status">{status}</strong>}
        </footer>
      </section>
    </div>
  );
}

function ModelDetail(props: {
  model: ModelInfo;
  isDefault: boolean;
  busy: string | null;
  onTest(): void;
  onDefault(): void;
  onRemove(): void;
}) {
  const meta = PROVIDERS[props.model.provider];
  return (
    <div className="model-detail">
      <div className="connection-heading detail-heading">
        <span className={`provider-mark large ${props.model.provider === "ollama" ? "local" : "remote"}`}>
          {meta.short}
        </span>
        <div>
          <span className="settings-kicker">CONFIGURED MODEL</span>
          <h3>{props.model.id}</h3>
          <p>{meta.title} · {props.isDefault ? "当前默认" : "可用于翻译与 Chat"}</p>
        </div>
        <span className={`connection-badge ${props.isDefault ? "enabled" : ""}`}>
          {props.isDefault ? "默认" : "已配置"}
        </span>
      </div>

      <dl className="model-facts">
        <div><dt>模型 ID</dt><dd>{props.model.model}</dd></div>
        <div><dt>API Base URL</dt><dd>{props.model.baseUrl}</dd></div>
        <div><dt>凭证</dt><dd>{props.model.apiKeyRef ? "已存入系统安全存储" : "未配置 API Key"}</dd></div>
        <div><dt>适配协议</dt><dd>{props.model.provider === "ollama" ? "Ollama / OpenAI endpoint" : "OpenAI-compatible"}</dd></div>
      </dl>

      <div className="detail-actions">
        <button type="button" onClick={props.onTest} disabled={props.busy === `test:${props.model.id}`}>
          {props.busy === `test:${props.model.id}` ? "测试中…" : "测试连接"}
        </button>
        {!props.isDefault && (
          <button type="button" className="primary-button" onClick={props.onDefault}>设为默认</button>
        )}
        <button type="button" className="danger-button" onClick={props.onRemove}>删除连接</button>
      </div>
    </div>
  );
}
