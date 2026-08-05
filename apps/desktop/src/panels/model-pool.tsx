// ModelPoolPanel — 最小模型池管理（P0 §7.1 / B1）。
// Ollama + OpenAI-compatible；增删、设默认、连接测试。
// Key 通过 secret command 写入 SecretStore，前端只持有 secretRef（验收 18）。
import { useState } from "react";
import type { DesktopTransport } from "../types";

export type ModelInfo = {
  id: string;
  provider: "ollama" | "openai-compatible";
  baseUrl: string;
  model: string;
  apiKeyRef?: string | null;
};

export function ModelPoolPanel(props: {
  transport: DesktopTransport;
  open: boolean;
  onClose(): void;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [defaultId, setDefaultId] = useState<string | null>(null);
  const [status, setStatus] = useState<string>("");
  const [form, setForm] = useState({ id: "", provider: "ollama", baseUrl: "", model: "", apiKey: "" });
  const [testing, setTesting] = useState<string | null>(null);

  if (!props.open) return null;

  async function refresh() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const list = await invoke<{ default: string; models: ModelInfo[] }>("workbench_models_list");
      setModels(list.models ?? []);
      setDefaultId(list.default ?? null);
      setStatus("");
    } catch (e) {
      setStatus(`读取模型配置失败：${String(e)}`);
    }
  }

  // 懒加载：首次打开时刷新（Web 测试环境无 Tauri 时静默降级）
  if (models.length === 0 && status === "") {
    void refresh();
  }

  async function addModel() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      let apiKeyRef: string | null = null;
      if (form.provider === "openai-compatible" && form.apiKey.trim()) {
        const r = await invoke<{ secretRef: string }>("workbench_secret_set", {
          service: "codelattice",
          account: form.id,
          secret: form.apiKey,
        });
        apiKeyRef = r.secretRef;
      }
      await invoke("workbench_models_add", {
        config: {
          id: form.id,
          provider: form.provider,
          baseUrl: form.baseUrl,
          model: form.model,
          apiKeyRef,
        },
      });
      setStatus("已添加模型");
      setForm({ id: "", provider: "ollama", baseUrl: "", model: "", apiKey: "" });
      await refresh();
    } catch (e) {
      setStatus(`添加失败：${String(e)}`);
    }
  }

  async function removeModel(id: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const r = await invoke<{ purgedCacheEntries: number }>("workbench_models_remove", { id });
      setStatus(`已删除 ${id}（清理缓存条目 ${r.purgedCacheEntries}）`);
      await refresh();
    } catch (e) {
      setStatus(`删除失败：${String(e)}`);
    }
  }

  async function setDefault(id: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("workbench_models_set_default", { id });
      setDefaultId(id);
    } catch (e) {
      setStatus(`设默认失败：${String(e)}`);
    }
  }

  async function testModel(id: string) {
    setTesting(id);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const r = await invoke<{ ok: boolean; detail: string }>("workbench_models_test", { id });
      setStatus(`测试 ${id}：${r.ok ? "OK" : "失败"}（${r.detail}）`);
    } catch (e) {
      setStatus(`测试失败：${String(e)}`);
    } finally {
      setTesting(null);
    }
  }

  return (
    <section className="panel model-pool" data-testid="model-pool">
      <h2>
        模型池 <button type="button" onClick={props.onClose} data-testid="model-pool-close">关闭</button>
      </h2>
      {status && <p className="hint">{status}</p>}

      <ul className="model-list" data-testid="model-list">
        {models.map((m) => (
          <li key={m.id} className="model-item" data-testid={`model-${m.id}`}>
            <span>
              <strong>{m.id}</strong>（{m.provider}）{m.id === defaultId && " · 默认"}
              <br />
              <small>
                {m.baseUrl} / {m.model}
                {m.apiKeyRef ? " · key: ref" : " · 无 Key"}
              </small>
            </span>
            <span className="model-actions">
              <button type="button" onClick={() => testModel(m.id)} disabled={testing === m.id}>
                {testing === m.id ? "测试中…" : "测试"}
              </button>
              {m.id !== defaultId && (
                <button type="button" onClick={() => setDefault(m.id)}>设默认</button>
              )}
              <button type="button" onClick={() => removeModel(m.id)}>删除</button>
            </span>
          </li>
        ))}
        {models.length === 0 && <li className="hint">尚未配置模型。添加 Ollama 或 OpenAI-compatible 端点。</li>}
      </ul>

      <div className="model-form" data-testid="model-form">
        <input
          placeholder="模型 id（如 qwen-local）"
          value={form.id}
          onChange={(e) => setForm({ ...form, id: e.target.value })}
        />
        <select
          value={form.provider}
          onChange={(e) => setForm({ ...form, provider: e.target.value })}
        >
          <option value="ollama">ollama</option>
          <option value="openai-compatible">openai-compatible</option>
        </select>
        <input
          placeholder="baseUrl（如 http://127.0.0.1:11434/v1）"
          value={form.baseUrl}
          onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
        />
        <input
          placeholder="model（如 qwen3:14b）"
          value={form.model}
          onChange={(e) => setForm({ ...form, model: e.target.value })}
        />
        {form.provider === "openai-compatible" && (
          <input
            type="password"
            placeholder="API Key（写入系统钥匙串，不落盘）"
            value={form.apiKey}
            onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
          />
        )}
        <button type="button" onClick={() => void addModel()} data-testid="model-add">
          添加
        </button>
      </div>
      <p className="hint">
        Key 经系统安全存储保存，前端只保存 secretRef；远程模型只发送结构化图证据，路径默认脱敏。
      </p>
    </section>
  );
}
