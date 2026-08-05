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
}
