// FakeDesktopTransport — in-memory 测试实现（P0 §5.1：生产 Tauri / 测试 fake）。
// 可选注入错误行为以验证静态降级路径。
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

export type FakeBehavior = {
  failEvidenceQueries?: boolean;
  failModel?: boolean;
  failSnapshotLoad?: boolean;
};

export class FakeDesktopTransport implements DesktopTransport {
  snapshots: SnapshotMeta[] = [];
  private snapshotData = new Map<string, SnapshotData>();
  private explainCalls = 0;
  private chatCalls = 0;
  private cancelled: string[] = [];

  constructor(
    snapshotJson: string,
    meta: SnapshotMeta,
    private readonly behavior: FakeBehavior = {},
  ) {
    this.snapshots = [meta];
    const data = JSON.parse(snapshotJson) as SnapshotData;
    this.snapshotData.set(meta.id, data);
  }

  stats() {
    return { explainCalls: this.explainCalls, chatCalls: this.chatCalls, cancelled: [...this.cancelled] };
  }

  async listSnapshots(): Promise<SnapshotMeta[]> {
    return [...this.snapshots];
  }

  async loadSnapshot(snapshotId: string): Promise<SnapshotData> {
    if (this.behavior.failSnapshotLoad) throw new Error("snapshot load failed (fake)");
    const d = this.snapshotData.get(snapshotId);
    if (!d) throw new Error(`snapshot not found: ${snapshotId}`);
    return d;
  }

  async getNodeContext(snapshotId: string, _nodeId: string): Promise<NodeContextBundle> {
    if (this.behavior.failEvidenceQueries) throw new Error("query service unavailable (fake)");
    const d = this.snapshotData.get(snapshotId);
    if (!d) throw new Error(`snapshot not found: ${snapshotId}`);
    return {
      schemaVersion: "codelattice.nodeContext.v1",
      snapshotId,
      nodeId: "n:fake",
      directCallers: [],
      directCallees: [],
      sourceRefs: [],
      limitations: [],
      coverageContext: {
        scope: "project", resolvedCalls: 0, totalCalls: 0,
        resolutionRate: 0, knownIncomplete: true, caveatRef: "coverage:project:calls",
      },
      origin: "full",
    };
  }

  async getEdgeEvidence(snapshotId: string, relationKey: string): Promise<EdgeEvidenceBundle> {
    if (this.behavior.failEvidenceQueries) throw new Error("query service unavailable (fake)");
    const d = this.snapshotData.get(snapshotId);
    if (!d) throw new Error(`snapshot not found: ${snapshotId}`);
    return {
      schemaVersion: "codelattice.edgeEvidence.v1",
      snapshotId,
      selection: { relationKey, occurrenceKey: null, sourceId: "n:a", targetId: "n:b", kind: "calls" },
      directUpstream: [],
      directDownstream: [],
      dependencyReach: [],
      sourceRefs: [],
      limitations: [],
      coverageContext: {
        scope: "project", resolvedCalls: 0, totalCalls: 0,
        resolutionRate: 0, knownIncomplete: true, caveatRef: "coverage:project:calls",
      },
      generatedFrom: { staticAnalysis: true, runtimeVerified: false, coverageVerified: false },
      origin: "full",
    };
  }

  async getCallChain(
    snapshotId: string,
    nodeId: string,
    direction: "upstream" | "downstream",
    _depth: number,
  ): Promise<CallChainResult> {
    if (this.behavior.failEvidenceQueries) throw new Error("query service unavailable (fake)");
    return {
      schemaVersion: "codelattice.callChain.v1",
      snapshotId,
      nodeId,
      direction,
      steps: [],
      truncated: false,
      coverageContext: {
        scope: "project", resolvedCalls: 0, totalCalls: 0,
        resolutionRate: 0, knownIncomplete: true, caveatRef: "coverage:project:calls",
      },
      origin: "full",
    };
  }

  async explainSelection(_req: ExplainRequest): Promise<StreamHandle> {
    this.explainCalls += 1;
    if (this.behavior.failModel) throw new Error("model unavailable (fake)");
    return this.stream(["answer-complete"]);
  }

  async chat(_req: ChatRequest): Promise<StreamHandle> {
    this.chatCalls += 1;
    if (this.behavior.failModel) throw new Error("model unavailable (fake)");
    return this.stream(["answer-chunk", "answer-complete"]);
  }

  async cancel(requestId: string): Promise<void> {
    this.cancelled.push(requestId);
  }

  async writeSmokeReport(_payload: Record<string, unknown>): Promise<void> {
    // no-op in tests
  }

  // ── 返工新增方法（fake 实现）─────────────────────────────────────────

  async selectProjectDirectory(): Promise<string> {
    return "/fake/project";
  }

  async analyze(_root: string, _language: string): Promise<{ jobId: string }> {
    return { jobId: "job-fake-1" };
  }

  async analyzeStatus(): Promise<{ state: string; jobId: string | null }> {
    return { state: "Completed", jobId: "job-fake-1" };
  }

  async analyzeCancel(): Promise<void> {}

  async pinSnapshot(_snapshotId: string): Promise<void> {}

  async unpinSnapshot(_snapshotId: string): Promise<void> {}

  async sessionCreate(_snapshotId: string): Promise<string> {
    return "sess:fake:1";
  }

  async sessionPin(_sessionId: string, _scopeType: string, _scopeId: string, _snapshotId: string): Promise<void> {}

  async sessionClose(_sessionId: string): Promise<void> {}

  async modelsList(): Promise<{ default: string; models: unknown[] }> {
    return { default: "", models: [] };
  }

  async modelsAdd(_config: Record<string, unknown>): Promise<void> {}

  async modelsRemove(_id: string): Promise<void> {}

  async modelsSetDefault(_id: string): Promise<void> {}

  async modelsTest(_id: string): Promise<{ ok: boolean; detail: string }> {
    return { ok: true, detail: "fake ok" };
  }

  async secretSet(_service: string, _account: string, _secret: string): Promise<{ secretRef: string }> {
    return { secretRef: "keychain:fake/account" };
  }

  async secretDelete(_secretRef: string): Promise<void> {}

  // ── stream ────────────────────────────────────────────────────────────

  private async stream(kinds: string[]): Promise<StreamHandle> {
    const requestId = `req:fake:${this.explainCalls + this.chatCalls}`;
    const events: GatewayEvent[] = kinds.map((k) => {
      if (k === "answer-chunk") return { kind: "answer-chunk", text: "fake", requestId };
      return {
        kind: "answer-complete",
        requestId,
        answer: {
          schemaVersion: "codelattice.understandingAnswer.v1",
          scope: { type: "project", id: "p" },
          answerSummary: "fake answer",
          claims: [],
          navigationActions: [],
        },
      };
    });
    const self = this;
    return {
      requestId,
      events: (async function* () {
        for (const e of events) yield e;
      })(),
      cancel: () => self.cancel(requestId),
    };
  }
}
