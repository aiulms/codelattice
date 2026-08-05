// DesktopTransport — 类型化传输接口（返工第二轮 B-fix）。
//
// 关键修复：
// - 每请求独立状态对象 {requestId, unsubscribe, terminated, waiters, queue}
// - cleanup 按 requestId 幂等；旧 generator finally 不影响新请求
// - 替换时 cancel 后端旧请求 + 终止旧 iterator + unsubscribe 旧 listener
// - generator 在 terminal 事件后 return
// - invoke 失败推入 error 事件并终止
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
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

/** 判定事件是否为终止信号。 */
function isTerminal(ev: GatewayEvent): boolean {
  return ev.kind === "answer-complete" || ev.kind === "error" || ev.kind === "budget-limit";
}

/** 每请求独立状态。 */
interface RequestState {
  requestId: string;
  unsubscribe: (() => void) | null;
  terminated: boolean;
  queue: GatewayEvent[];
  waiters: Array<(e: GatewayEvent | null) => void>;
}

export class TauriDesktopTransport implements DesktopTransport {
  private currentRequest: RequestState | null = null;
  private requestSequence = 0;

  async listSnapshots(): Promise<SnapshotMeta[]> {
    return invoke<SnapshotMeta[]>("workbench_list_snapshots");
  }
  async loadSnapshot(snapshotId: string): Promise<SnapshotData> {
    return invoke<SnapshotData>("workbench_load_snapshot", { snapshotId });
  }
  async getNodeContext(snapshotId: string, nodeId: string): Promise<NodeContextBundle> {
    return invoke<NodeContextBundle>("workbench_node_context", { snapshotId, nodeId });
  }
  async getEdgeEvidence(snapshotId: string, relationKey: string, occurrenceKey?: string): Promise<EdgeEvidenceBundle> {
    return invoke<EdgeEvidenceBundle>("workbench_edge_evidence", { snapshotId, relationKey, occurrenceKey: occurrenceKey ?? null });
  }
  async getCallChain(snapshotId: string, nodeId: string, direction: "upstream" | "downstream", depth: number): Promise<CallChainResult> {
    return invoke<CallChainResult>("workbench_call_chain", { snapshotId, nodeId, direction, depth });
  }
  async selectProjectDirectory(): Promise<string> {
    const selected = await open({ directory: true, multiple: false });
    return typeof selected === "string" ? selected : "";
  }
  async analyze(root: string, language: string): Promise<{ jobId: string }> {
    return invoke<{ jobId: string }>("workbench_analyze", { root, language });
  }
  async analyzeStatus(): Promise<{ state: string; jobId: string | null; publishedSnapshotId?: string | null; error?: string | null }> {
    return invoke("workbench_analyze_status");
  }
  async analyzeCancel(): Promise<void> { await invoke("workbench_analyze_cancel"); }
  async pinSnapshot(snapshotId: string): Promise<void> { await invoke("workbench_pin_snapshot", { snapshotId }); }
  async unpinSnapshot(snapshotId: string): Promise<void> { await invoke("workbench_unpin_snapshot", { snapshotId }); }
  async sessionCreate(snapshotId: string): Promise<string> {
    return invoke<string>("workbench_session_create", { snapshotId });
  }
  async sessionPin(sessionId: string, scopeType: string, scopeId: string, snapshotId: string): Promise<void> {
    await invoke("workbench_session_pin", { sessionId, scopeType, scopeId, snapshotId });
  }
  async sessionClose(sessionId: string): Promise<void> { await invoke("workbench_session_close", { sessionId }); }
  async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return invoke<{ default: string; models: unknown[] }>("workbench_models_list");
  }
  async modelsAdd(config: Record<string, unknown>): Promise<void> { await invoke("workbench_models_add", { config }); }
  async modelsRemove(id: string): Promise<void> { await invoke("workbench_models_remove", { id }); }
  async modelsSetDefault(id: string): Promise<void> { await invoke("workbench_models_set_default", { id }); }
  async modelsTest(id: string): Promise<{ ok: boolean; detail: string }> {
    return invoke<{ ok: boolean; detail: string }>("workbench_models_test", { id });
  }
  async secretSet(service: string, account: string, secret: string): Promise<{ secretRef: string }> {
    return invoke<{ secretRef: string }>("workbench_secret_set", { service, account, secret });
  }
  async secretDelete(secretRef: string): Promise<void> { await invoke("workbench_secret_delete", { secretRef }); }
  async explainSelection(req: ExplainRequest): Promise<StreamHandle> {
    const requestId = this.nextRequestId("exp");
    return this.openStream(requestId, "workbench_explain", req);
  }
  async chat(req: ChatRequest): Promise<StreamHandle> {
    const requestId = this.nextRequestId("chat");
    return this.openStream(requestId, "workbench_chat", req);
  }
  async cancel(requestId: string): Promise<void> { await invoke("workbench_cancel", { requestId }); }
  async writeSmokeReport(payload: Record<string, unknown>): Promise<void> { await invoke("workbench_smoke_report", { payload }); }

  /**
   * 启动命令并订阅事件。每请求独立状态；替换时先 cancel 后端旧请求。
   */
  private async openStream(requestId: string, command: string, payload: unknown): Promise<StreamHandle> {
    // 替换旧请求：cancel 后端 + 终止旧 iterator + unsubscribe
    if (this.currentRequest && !this.currentRequest.terminated) {
      const oldRid = this.currentRequest.requestId;
      // cancel 后端旧请求
      await invoke("workbench_cancel", { requestId: oldRid }).catch(() => {});
      // 终止旧请求状态
      this.disposeRequest(oldRid);
    }

    const rs: RequestState = {
      requestId,
      unsubscribe: null,
      terminated: false,
      queue: [],
      waiters: [],
    };

    const unsub = await listen<GatewayEvent>(`gateway:${requestId}`, (evt) => {
      const e = evt.payload;
      if (rs.waiters.length > 0) {
        rs.waiters.shift()!(e);
      } else {
        rs.queue.push(e);
      }
    });
    rs.unsubscribe = unsub;
    this.currentRequest = rs;

    // invoke 可能失败
    invoke(command, { requestId, ...(payload as Record<string, unknown>) }).catch((err) => {
      const errorEvent: GatewayEvent = { kind: "error", message: String(err?.message ?? err), requestId };
      if (rs.waiters.length > 0) {
        rs.waiters.shift()!(errorEvent);
      } else {
        rs.queue.push(errorEvent);
      }
    });

    const self = this;
    return {
      requestId,
      events: (async function* (): AsyncGenerator<GatewayEvent> {
        try {
          while (true) {
            let ev: GatewayEvent | null = null;
            if (rs.queue.length > 0) {
              ev = rs.queue.shift()!;
            } else {
              ev = await new Promise<GatewayEvent | null>((res) => rs.waiters.push(res));
            }
            if (ev === null) return; // disposed
            yield ev;
            if (isTerminal(ev)) return; // terminal event
          }
        } finally {
          // 只清理自己对应的请求，不误清理新请求
          self.disposeRequest(requestId);
        }
      })(),
      cancel: async () => {
        try {
          await self.cancel(requestId);
        } finally {
          // 后端即使没有及时返回 terminal，也必须结束本地 generator 并释放 listener。
          self.disposeRequest(requestId);
        }
      },
    };
  }

  /** 同一毫秒内也保持唯一，避免并发请求覆盖 active_requests。 */
  private nextRequestId(kind: "exp" | "chat"): string {
    this.requestSequence += 1;
    return `req:${Date.now().toString(36)}:${this.requestSequence.toString(36)}:${kind}`;
  }

  /** 清理指定请求（幂等）。只清理匹配的请求，不影响新请求。 */
  private disposeRequest(requestId: string): void {
    // 只有当 currentRequest 匹配时才清理
    if (this.currentRequest && this.currentRequest.requestId === requestId) {
      const rs = this.currentRequest;
      if (!rs.terminated) {
        rs.terminated = true;
        // 唤醒所有等待中的 waiter
        while (rs.waiters.length > 0) {
          rs.waiters.shift()!(null);
        }
        rs.queue.length = 0;
        if (rs.unsubscribe) {
          rs.unsubscribe();
          rs.unsubscribe = null;
        }
      }
      this.currentRequest = null;
    }
  }
}
