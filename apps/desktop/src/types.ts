// CodeLattice Workbench — shared DTO contracts (P0 §6, v4 execution card).
// All layers (UI, transport, gateway) exchange these shapes. Business modules
// must never depend on `window.__TAURI__`, HTTP URLs or hardcoded local paths.

// ── Selection model (§4.1) ──────────────────────────────────────────────────

export type GraphSelection =
  | { type: "none" }
  | { type: "node"; nodeId: string; snapshotId: string }
  | { type: "relation"; relationKey: string; occurrenceKey?: string; snapshotId: string }
  | { type: "chain"; chainId: string; snapshotId: string }
  | { type: "multi"; snapshotId: string; nodeIds: string[]; relationKeys: string[] };

export type ConversationScopeType = "project" | "node" | "edge" | "chain";

export type ConversationContext = {
  sessionId: string;
  pinnedScope: { type: ConversationScopeType; id: string } | null;
  snapshotId: string;
  stale: boolean;
};

// ── Fact snapshot (webui.snapshot.v1 + P0 back-compat additions) ────────────

export type SnapshotNode = {
  id: string;
  label: string;
  kind: "symbol" | "file" | "package" | "entry" | "risk" | "related" | string;
  file?: string;
  line?: number;
  visibility?: string;
  /** 节点语言身份（快照生产方标注）。缺席即未知——契约禁止 ""/null。 */
  language?: string;
};

export type SnapshotEdge = {
  source: string;
  target: string;
  kind: "calls" | "imports" | "defines" | "owns" | "related" | string;
  confidence?: number;
  reason?: string;
  /** 聚合视图（模块/文件）上的底层边数；符号级图不出现。 */
  count?: number;
  // P0 identity additions (§6.1): semantic relation identity vs call-site identity.
  // occurrenceKey only exists when the fact layer provides a stable call-site
  // designation; otherwise the UI must stay relation-level.
  relationKey?: string;
  occurrenceKey?: string | null;
};

export type CoverageContext = {
  scope: "project" | "module";
  resolvedCalls: number;
  totalCalls: number;
  resolutionRate: number;
  knownIncomplete: boolean;
  caveatRef: string;
};

export type StaticLimitation = {
  id: string;
  text: string;
};

export type GraphSummary = {
  nodeCount: number;
  edgeCount: number;
  fileNodeCount: number;
  symbolNodeCount: number;
  callEdgeCount: number;
};

export type ModuleGraphModule = {
  id: string;
  files: number;
  symbols: number;
  /** 模块内节点语言并集（字母序去重）。全未知时字段缺席——契约禁止 []。 */
  languages?: string[];
};

export type ModuleGraphEdge = {
  source: string;
  target: string;
  count: number;
  kinds: string[];
  minConfidence?: number;
  reasons?: string[];
};

export type ModuleGraph = {
  modules: ModuleGraphModule[];
  edges: ModuleGraphEdge[];
  truncated: boolean;
};

export type GraphLevel = "module" | "file" | "symbol";

export type SnapshotData = {
  schemaVersion: string;
  generatedAt: string;
  generatedFrom: { staticAnalysis: boolean; runtimeVerified: boolean };
  summary: Record<string, unknown>;
  graph: {
    status: string;
    stability: string;
    nodes: SnapshotNode[];
    edges: SnapshotEdge[];
    summary: GraphSummary;
    truncated: boolean;
    cautions: string[];
  };
  moduleGraph?: ModuleGraph;
  limitations: { notes: string[] } | string[];
  insights?: { entryPoints?: unknown[]; hotspots?: unknown[]; status?: string };
  explore?: { symbols?: unknown[]; sourceFiles?: unknown[]; truncated?: boolean };
};

export type SnapshotMeta = {
  id: string;
  rootLabel: string;
  language: string;
  createdAt: string;
  summary?: { symbolCount?: number; sourceFileCount?: number };
};

// ── On-demand evidence bundle (§6.2, codelattice.edgeEvidence.v1) ───────────

export type SourceRef = {
  id: string;
  file: string;
  startLine: number;
  endLine: number;
};

export type RelationRef = {
  relationKey: string;
  occurrenceKey: string | null;
  sourceId: string;
  targetId: string;
  kind: string;
};

export type EdgeEvidenceBundle = {
  schemaVersion: "codelattice.edgeEvidence.v1";
  snapshotId: string;
  selection: RelationRef;
  directUpstream: RelationRef[];
  directDownstream: RelationRef[];
  dependencyReach: RelationRef[];
  sourceRefs: SourceRef[];
  limitations: StaticLimitation[];
  coverageContext: CoverageContext;
  generatedFrom: {
    staticAnalysis: boolean;
    runtimeVerified: boolean;
    coverageVerified: boolean;
  };
  // "preview" = derived from the bounded snapshot only; "full" = on-demand query
  origin: "preview" | "full";
};

export type NodeContextBundle = {
  schemaVersion: "codelattice.nodeContext.v1";
  snapshotId: string;
  nodeId: string;
  directCallers: RelationRef[];
  directCallees: RelationRef[];
  sourceRefs: SourceRef[];
  limitations: StaticLimitation[];
  coverageContext: CoverageContext;
  origin: "preview" | "full";
};

export type CallChainResult = {
  schemaVersion: "codelattice.callChain.v1";
  snapshotId: string;
  nodeId: string;
  direction: "upstream" | "downstream";
  steps: Array<{ relation: RelationRef; sourceRef?: SourceRef; confidence?: number }>;
  truncated: boolean;
  coverageContext: CoverageContext;
  origin: "preview" | "full";
};

// ── Model output (§6.3, codelattice.understandingAnswer.v1) ─────────────────

export type ClaimClassification = "grounded_interpretation" | "hypothesis" | "unknown";

export type Claim = {
  id: string;
  text: string;
  classification: ClaimClassification;
  evidenceRefs: string[];
  coverageCaveatRefs: string[];
};

export type NavigationAction =
  | { type: "focusNode"; nodeId: string; snapshotId: string }
  | { type: "focusRelation"; relationKey: string; snapshotId: string; occurrenceKey?: string }
  | { type: "focusSource"; sourceRefId: string; snapshotId: string };

export type UnderstandingAnswer = {
  schemaVersion: "codelattice.understandingAnswer.v1";
  scope: { type: ConversationScopeType; id: string };
  answerSummary: string;
  claims: Claim[];
  navigationActions: NavigationAction[];
};

// ── Chat / streaming events ─────────────────────────────────────────────────

export type GatewayEvent =
  | { kind: "answer-chunk"; text: string; requestId: string }
  | { kind: "answer-complete"; answer: UnderstandingAnswer; requestId: string }
  | { kind: "tool-call"; trace: ToolTrace; requestId: string }
  | { kind: "budget-limit"; reason: string; requestId: string }
  | { kind: "error"; message: string; requestId: string };

export type ToolTrace = {
  tool: string;
  params: Record<string, unknown>;
  returnedBytes: number;
  truncated: boolean;
};

// ── Workspace inspection (codelattice.workspaceInspection.v1, 多语言卡 2/3) ──

export type InspectionEvidence = {
  kind: "manifest" | "extension-histogram";
  file?: string;
  extension?: string;
  count?: number;
};

export type InspectionRow = {
  name?: string;
  relativePath: string;
  /** 缺席即未知：unrecognized 行没有 language（契约禁止 ""/null）。 */
  language?: string;
  confidence: string;
  evidence: InspectionEvidence;
  sourceFileCount: number;
  analyzable: boolean;
  reason?: string;
  recognition?: string;
};

export type WorkspaceInspection = {
  schemaVersion: string;
  root?: string;
  generatedAt?: string;
  generatedFrom?: {
    staticAnalysis: boolean;
    projectContentRead: boolean;
    scriptsExecuted: boolean;
  };
  projects: InspectionRow[];
  sourceOnlyAreas: InspectionRow[];
  unsupportedAreas: InspectionRow[];
  cautions?: string[];
  recommendedNextActions?: string[];
};

// ── Transport (§5.1 / §5.3) ─────────────────────────────────────────────────

export type StreamHandle = {
  requestId: string;
  events: AsyncIterable<GatewayEvent>;
  cancel(): Promise<void>;
};

export type ExplainRequest = {
  selection: GraphSelection;
  providerId: string;
  explanationLevel: "brief" | "detailed";
};

export type ChatRequest = {
  sessionId: string;
  message: string;
  providerId: string;
  /** 返工第二轮：snapshotId 和 pinnedScope 必填（禁止后端回退）。 */
  snapshotId: string;
  pinnedScope: { type: ConversationScopeType; id: string } | null;
};

export interface DesktopTransport {
  listSnapshots(): Promise<SnapshotMeta[]>;
  loadSnapshot(snapshotId: string): Promise<SnapshotData>;
  getNodeContext(snapshotId: string, nodeId: string): Promise<NodeContextBundle>;
  getEdgeEvidence(
    snapshotId: string,
    relationKey: string,
    occurrenceKey?: string,
  ): Promise<EdgeEvidenceBundle>;
  getCallChain(
    snapshotId: string,
    nodeId: string,
    direction: "upstream" | "downstream",
    depth: number,
  ): Promise<CallChainResult>;
  explainSelection(req: ExplainRequest): Promise<StreamHandle>;
  chat(req: ChatRequest): Promise<StreamHandle>;
  cancel(requestId: string): Promise<void>;
  writeSmokeReport(payload: Record<string, unknown>): Promise<void>;
  // 返工新增：Analyzer / session / model pool 类型化方法
  selectProjectDirectory(): Promise<string>;
  inspect(root: string): Promise<WorkspaceInspection>;
  analyze(root: string, language: string): Promise<{ jobId: string }>;
  analyzeStatus(): Promise<{ state: string; jobId: string | null; publishedSnapshotId?: string | null; error?: string | null }>;
  analyzeCancel(): Promise<void>;
  pinSnapshot(snapshotId: string): Promise<void>;
  unpinSnapshot(snapshotId: string): Promise<void>;
  sessionCreate(snapshotId: string): Promise<string>;
  sessionPin(sessionId: string, scopeType: string, scopeId: string, snapshotId: string): Promise<void>;
  sessionClose(sessionId: string): Promise<void>;
  modelsList(): Promise<{ default: string; models: unknown[] }>;
  modelsAdd(config: Record<string, unknown>): Promise<void>;
  modelsUpdate(config: Record<string, unknown>): Promise<void>;
  modelsRemove(id: string): Promise<void>;
  modelsSetDefault(id: string): Promise<void>;
  modelsTest(id: string): Promise<{ ok: boolean; detail: string }>;
  secretSet(service: string, account: string, secret: string): Promise<{ secretRef: string }>;
  secretDelete(secretRef: string): Promise<void>;
}
