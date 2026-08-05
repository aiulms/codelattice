// DesktopTransport — 类型化传输接口（P0 §5.1）。
//
// 生产实现 = Tauri commands/channels；测试实现 = in-memory fake；
// 兼容 Web adapter（HTTP/SSE）另行实现。业务模块绝不直接调用
// `window.__TAURI__`（组件禁止触碰 Tauri 全局对象，F1 规则）。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CallChainResult,
  ChatRequest,
  DesktopTransport,
  EdgeEvidenceBundle,
  ExplainRequest,
  GatewayEvent,
  NodeContextBundle,
  SnapshotData,
  SnapshotMeta,
  StreamHandle,
} from "../types";

export class TauriDesktopTransport implements DesktopTransport {
  private eventUnsubs: Array<() => void> = [];

  async listSnapshots(): Promise<SnapshotMeta[]> {
    return invoke<SnapshotMeta[]>("workbench_list_snapshots");
  }

  async loadSnapshot(snapshotId: string): Promise<SnapshotData> {
    return invoke<SnapshotData>("workbench_load_snapshot", { snapshotId });
  }

  async getNodeContext(snapshotId: string, nodeId: string): Promise<NodeContextBundle> {
    return invoke<NodeContextBundle>("workbench_node_context", { snapshotId, nodeId });
  }

  async getEdgeEvidence(
    snapshotId: string,
    relationKey: string,
    occurrenceKey?: string,
  ): Promise<EdgeEvidenceBundle> {
    return invoke<EdgeEvidenceBundle>("workbench_edge_evidence", {
      snapshotId, relationKey, occurrenceKey: occurrenceKey ?? null,
    });
  }

  async getCallChain(
    snapshotId: string,
    nodeId: string,
    direction: "upstream" | "downstream",
    depth: number,
  ): Promise<CallChainResult> {
    return invoke<CallChainResult>("workbench_call_chain", {
      snapshotId, nodeId, direction, depth,
    });
  }

  async explainSelection(req: ExplainRequest): Promise<StreamHandle> {
    const requestId = `req:${Date.now().toString(36)}`;
    return this.openStream(requestId, "workbench_explain", req);
  }

  async chat(req: ChatRequest): Promise<StreamHandle> {
    const requestId = `req:${Date.now().toString(36)}`;
    return this.openStream(requestId, "workbench_chat", req);
  }

  async cancel(requestId: string): Promise<void> {
    await invoke("workbench_cancel", { requestId });
  }

  async writeSmokeReport(payload: Record<string, unknown>): Promise<void> {
    await invoke("workbench_smoke_report", { payload });
  }

  /** 启动命令，并订阅该 requestId 的流式事件 channel（验收 25：streaming/cancel）。 */
  private async openStream(requestId: string, command: string, payload: unknown): Promise<StreamHandle> {
    const events: GatewayEvent[] = [];
    const waiters: Array<(e: GatewayEvent) => void> = [];
    const unsub = await listen<GatewayEvent>(`gateway:${requestId}`, (evt) => {
      const e = evt.payload;
      if (waiters.length > 0) waiters.shift()!(e);
      else events.push(e);
    });
    this.eventUnsubs.push(unsub);
    await invoke(command, { requestId, ...(payload as Record<string, unknown>) });

    const self = this;
    return {
      requestId,
      events: (async function* () {
        while (true) {
          if (events.length > 0) yield events.shift()!;
          else {
            const e = await new Promise<GatewayEvent>((res) => waiters.push(res));
            yield e;
          }
        }
      })(),
      cancel: () => self.cancel(requestId),
    };
  }
}
