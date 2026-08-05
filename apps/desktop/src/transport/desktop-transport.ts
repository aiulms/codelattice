// DesktopTransport — 类型化传输接口（P0 §5.1）。
//
// 生产实现 = Tauri commands/channels；测试实现 = in-memory fake；
// 兼容 Web adapter（HTTP/SSE）另行实现。业务模块绝不直接调用
// `window.__TAURI__`（组件禁止触碰 Tauri 全局对象，F1 规则）。
//
// 返工修复：
// - requestId 贯穿 UI message、Tauri command、event topic、cancel；
// - generator 在 answer-complete/error/cancel/budget-limit 后终止；
// - 所有终止路径都 unsubscribe listener + 清空 queue；
// - invoke 失败时也清理 listener；
// - 同一时间单活动请求策略。
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

/** 判定事件是否为终止信号（generator 应在此后停止 yield）。 */
function isTerminal(ev: GatewayEvent): boolean {
  return (
    ev.kind === "answer-complete" ||
    ev.kind === "error" ||
    ev.kind === "budget-limit"
  );
}

export class TauriDesktopTransport implements DesktopTransport {
  /** 当前活动请求的 requestId（单活动请求策略；disposeActive 用闭包访问）。 */
  /** 活动请求的 unsubscribe（用于 cancel / 替换 / 错误清理）。 */
  private activeUnsub: (() => void) | null = null;

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

  // ── Analyzer / snapshot management ─────────────────────────────────────

  async selectProjectDirectory(): Promise<string> {
    return invoke<string>("workbench_select_directory");
  }

  async analyze(root: string, language: string): Promise<{ jobId: string }> {
    return invoke<{ jobId: string }>("workbench_analyze", { root, language });
  }

  async analyzeStatus(): Promise<{ state: string; jobId: string | null }> {
    return invoke<{ state: string; jobId: string | null }>("workbench_analyze_status");
  }

  async analyzeCancel(): Promise<void> {
    await invoke("workbench_analyze_cancel");
  }

  async pinSnapshot(snapshotId: string): Promise<void> {
    await invoke("workbench_pin_snapshot", { snapshotId });
  }

  async unpinSnapshot(snapshotId: string): Promise<void> {
    await invoke("workbench_unpin_snapshot", { snapshotId });
  }

  // ── Session management ─────────────────────────────────────────────────

  async sessionCreate(snapshotId: string): Promise<string> {
    return invoke<string>("workbench_session_create", { snapshotId });
  }

  async sessionPin(
    sessionId: string,
    scopeType: string,
    scopeId: string,
    snapshotId: string,
  ): Promise<void> {
    await invoke("workbench_session_pin", {
      sessionId, scopeType, scopeId, snapshotId,
    });
  }

  async sessionClose(sessionId: string): Promise<void> {
    await invoke("workbench_session_close", { sessionId });
  }

  // ── Model pool ─────────────────────────────────────────────────────────

  async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return invoke<{ default: string; models: unknown[] }>("workbench_models_list");
  }

  async modelsAdd(config: Record<string, unknown>): Promise<void> {
    await invoke("workbench_models_add", { config });
  }

  async modelsRemove(id: string): Promise<void> {
    await invoke("workbench_models_remove", { id });
  }

  async modelsSetDefault(id: string): Promise<void> {
    await invoke("workbench_models_set_default", { id });
  }

  async modelsTest(id: string): Promise<{ ok: boolean; detail: string }> {
    return invoke<{ ok: boolean; detail: string }>("workbench_models_test", { id });
  }

  async secretSet(service: string, account: string, secret: string): Promise<{ secretRef: string }> {
    return invoke<{ secretRef: string }>("workbench_secret_set", { service, account, secret });
  }

  async secretDelete(secretRef: string): Promise<void> {
    await invoke("workbench_secret_delete", { secretRef });
  }

  // ── Streaming ──────────────────────────────────────────────────────────

  async explainSelection(req: ExplainRequest): Promise<StreamHandle> {
    const requestId = `req:${Date.now().toString(36)}:exp`;
    return this.openStream(requestId, "workbench_explain", req);
  }

  async chat(req: ChatRequest): Promise<StreamHandle> {
    const requestId = `req:${Date.now().toString(36)}:chat`;
    return this.openStream(requestId, "workbench_chat", req);
  }

  async cancel(requestId: string): Promise<void> {
    await invoke("workbench_cancel", { requestId });
  }

  async writeSmokeReport(payload: Record<string, unknown>): Promise<void> {
    await invoke("workbench_smoke_report", { payload });
  }

  /**
   * 启动命令，订阅该 requestId 的流式事件 channel。
   *
   * 返工修复要点：
   * 1. 如果已有活动请求 → 先 cancel + 清理（替换策略）。
   * 2. listener 在 generator 终止 / cancel / invoke 失败时必须 unsubscribe。
   * 3. generator 在 answer-complete / error / budget-limit 后终止。
   */
  private async openStream(requestId: string, command: string, payload: unknown): Promise<StreamHandle> {
    // 替换策略：如果有活动请求，先取消并清理
    this.disposeActive();

    const events: GatewayEvent[] = [];
    const waiters: Array<(e: GatewayEvent | null) => void> = [];
    let terminated = false;

    const unsub = await listen<GatewayEvent>(`gateway:${requestId}`, (evt) => {
      const e = evt.payload;
      if (waiters.length > 0) {
        waiters.shift()!(e);
      } else {
        events.push(e);
      }
    });

    this.activeUnsub = () => {
      if (!terminated) {
        terminated = true;
        // 唤醒所有等待中的 waiter（传 null 表示结束）
        while (waiters.length > 0) {
          waiters.shift()!(null);
        }
        events.length = 0;
        unsub();
      }
    };

    // invoke 可能失败 → 必须清理 listener
    const invokeResult = invoke(command, { requestId, ...(payload as Record<string, unknown>) })
      .catch((err) => {
        // invoke 失败：推入 error 事件让 generator 正常终止
        const errorEvent: GatewayEvent = {
          kind: "error",
          message: String(err?.message ?? err),
          requestId,
        };
        if (waiters.length > 0) {
          waiters.shift()!(errorEvent);
        } else {
          events.push(errorEvent);
        }
      });

    const self = this;
    void invokeResult; // fire and forget — 错误经 event 处理

    return {
      requestId,
      events: (async function* (): AsyncGenerator<GatewayEvent> {
        try {
          while (true) {
            let ev: GatewayEvent | null = null;
            if (events.length > 0) {
              ev = events.shift()!;
            } else {
              ev = await new Promise<GatewayEvent | null>((res) => waiters.push(res));
            }
            if (ev === null) {
              // disposed (cancel / replace)
              return;
            }
            yield ev;
            if (isTerminal(ev)) {
              return;
            }
          }
        } finally {
          self.disposeActive();
        }
      })(),
      cancel: () => self.cancel(requestId),
    };
  }

  /** 清理活动请求：unsubscribe listener + 唤醒 waiter。 */
  private disposeActive(): void {
    if (this.activeUnsub) {
      this.activeUnsub();
      this.activeUnsub = null;
    }
  }
}
