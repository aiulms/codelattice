// HttpDesktopTransport — 兼容 Web Runner 的 HTTP/SSE 适配（P0 §5.2）。
// 只用于旧 Web 宿主/浏览器开发调试；桌面主链使用 Tauri commands/channels。
// 实现 ok/err DTO（F0 字符化冻结）与 JSON 错误信封。
import type {
  CallChainResult,
  ChatRequest,
  DesktopTransport,
  EdgeEvidenceBundle,
  ExplainRequest,
  NodeContextBundle,
  SnapshotData,
  SnapshotMeta,
  StreamHandle,
} from "../types";

export class HttpDesktopTransport implements DesktopTransport {
  constructor(private readonly baseUrl: string) {}

  private async get<T>(path: string): Promise<T> {
    const r = await fetch(`${this.baseUrl}${path}`);
    const body = await r.json().catch(() => null);
    if (!r.ok || !body?.success) {
      throw new Error(body?.error ?? `HTTP ${r.status}`);
    }
    return body.data as T;
  }

  async listSnapshots(): Promise<SnapshotMeta[]> {
    return this.get<SnapshotMeta[]>("/api/snapshots");
  }

  async loadSnapshot(snapshotId: string): Promise<SnapshotData> {
    return this.get<SnapshotData>(`/api/snapshot/${snapshotId}`);
  }

  async getNodeContext(_snapshotId: string, _nodeId: string): Promise<NodeContextBundle> {
    throw new Error("node context endpoint not available on legacy runner (P0-A)");
  }

  async getEdgeEvidence(_snapshotId: string, _relationKey: string): Promise<EdgeEvidenceBundle> {
    throw new Error("edge evidence endpoint not available on legacy runner (P0-A)");
  }

  async getCallChain(
    _snapshotId: string,
    _nodeId: string,
    _direction: "upstream" | "downstream",
    _depth: number,
  ): Promise<CallChainResult> {
    throw new Error("call chain endpoint not available on legacy runner (P0-A)");
  }

  async explainSelection(_req: ExplainRequest): Promise<StreamHandle> {
    throw new Error("model services are P0-B");
  }

  async chat(_req: ChatRequest): Promise<StreamHandle> {
    throw new Error("model services are P0-B");
  }

  async cancel(_requestId: string): Promise<void> {
    throw new Error("model services are P0-B");
  }

  async writeSmokeReport(_payload: Record<string, unknown>): Promise<void> {
    // 浏览器环境：写入 window 供外部读取
    (window as unknown as Record<string, unknown>).__smokeReport = _payload;
  }

  // ── 返工新增方法（旧 Web Runner 不支持，抛错降级）─────────────────────

  async selectProjectDirectory(): Promise<string> {
    throw new Error("not available on legacy runner");
  }

  async analyze(_root: string, _language: string): Promise<{ jobId: string }> {
    throw new Error("not available on legacy runner");
  }

  async analyzeStatus(): Promise<{ state: string; jobId: string | null }> {
    throw new Error("not available on legacy runner");
  }

  async analyzeCancel(): Promise<void> {
    throw new Error("not available on legacy runner");
  }

  async pinSnapshot(_snapshotId: string): Promise<void> {}

  async unpinSnapshot(_snapshotId: string): Promise<void> {}

  async sessionCreate(_snapshotId: string): Promise<string> {
    return `sess:web:${Date.now().toString(36)}`;
  }

  async sessionPin(_sessionId: string, _scopeType: string, _scopeId: string, _snapshotId: string): Promise<void> {}

  async sessionClose(_sessionId: string): Promise<void> {}

  async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return { default: "", models: [] };
  }

  async modelsAdd(_config: Record<string, unknown>): Promise<void> {
    throw new Error("not available on legacy runner");
  }

  async modelsRemove(_id: string): Promise<void> {}

  async modelsSetDefault(_id: string): Promise<void> {}

  async modelsTest(_id: string): Promise<{ ok: boolean; detail: string }> {
    throw new Error("not available on legacy runner");
  }

  async secretSet(_service: string, _account: string, _secret: string): Promise<{ secretRef: string }> {
    throw new Error("not available on legacy runner");
  }

  async secretDelete(_secretRef: string): Promise<void> {}
}
