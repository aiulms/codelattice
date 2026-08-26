//! DTO —— 与前端共享的序列化契约（P0 §6，JSON 可往返）。
//! UI / Tauri adapter / Web adapter 共享同一组 DTO；本模块不含业务逻辑。

use serde::{Deserialize, Serialize};

// ── Selection model（§4.1）─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GraphSelection {
    None,
    Node {
        node_id: String,
        snapshot_id: String,
    },
    Relation {
        relation_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        occurrence_key: Option<String>,
        snapshot_id: String,
    },
    Chain {
        chain_id: String,
        snapshot_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationScopeType {
    Project,
    Node,
    Edge,
    Chain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationContext {
    pub session_id: String,
    pub pinned_scope: Option<PinnedScope>,
    pub snapshot_id: String,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedScope {
    #[serde(rename = "type")]
    pub scope_type: ConversationScopeType,
    pub id: String,
}

// ── Edge evidence（§6.2，codelattice.edgeEvidence.v1）─────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationRef {
    pub relation_key: String,
    pub occurrence_key: Option<String>,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub id: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageContext {
    pub scope: String, // 只允许 "project"；模块级需要事实层 denominator
    pub resolved_calls: u64,
    pub total_calls: u64,
    pub resolution_rate: f64,
    pub known_incomplete: bool,
    pub caveat_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticLimitation {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeEvidenceBundle {
    pub schema_version: String,
    pub snapshot_id: String,
    pub selection: RelationRef,
    pub direct_upstream: Vec<RelationRef>,
    pub direct_downstream: Vec<RelationRef>,
    pub dependency_reach: Vec<RelationRef>,
    pub source_refs: Vec<SourceRef>,
    pub limitations: Vec<StaticLimitation>,
    pub coverage_context: CoverageContext,
    pub generated_from: GeneratedFrom,
    pub origin: EvidenceOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceOrigin {
    Preview,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFrom {
    pub static_analysis: bool,
    pub runtime_verified: bool,
    pub coverage_verified: bool,
}

// ── On-demand queries（§6.2 / P0-A，codelattice.nodeContext.v1 / callChain.v1）─

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeContextBundle {
    pub schema_version: String,
    pub snapshot_id: String,
    pub node_id: String,
    pub direct_callers: Vec<RelationRef>,
    pub direct_callees: Vec<RelationRef>,
    pub source_refs: Vec<SourceRef>,
    pub limitations: Vec<StaticLimitation>,
    pub coverage_context: CoverageContext,
    pub origin: EvidenceOrigin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStep {
    pub relation: RelationRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallChainResult {
    pub schema_version: String,
    pub snapshot_id: String,
    pub node_id: String,
    pub direction: String, // "upstream" | "downstream"
    pub steps: Vec<ChainStep>,
    pub truncated: bool,
    pub coverage_context: CoverageContext,
    pub origin: EvidenceOrigin,
}

// ── Model output（§6.3，codelattice.understandingAnswer.v1）────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimClassification {
    GroundedInterpretation,
    Hypothesis,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub classification: ClaimClassification,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    // §6.3：coverageCaveatRefs 由 Gateway 根据证据包补充，不要求模型生成。
    #[serde(default)]
    pub coverage_caveat_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NavigationAction {
    FocusNode {
        node_id: String,
        #[serde(default)]
        snapshot_id: String,
    },
    FocusRelation {
        relation_key: String,
        #[serde(default)]
        snapshot_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        occurrence_key: Option<String>,
    },
    FocusSource {
        source_ref_id: String,
        #[serde(default)]
        snapshot_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnderstandingAnswer {
    pub schema_version: String,
    pub scope: AnswerScope,
    pub answer_summary: String,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub navigation_actions: Vec<NavigationAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerScope {
    #[serde(rename = "type")]
    pub scope_type: ConversationScopeType,
    pub id: String,
}

// ── Gateway events（streaming）─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GatewayEvent {
    AnswerChunk {
        text: String,
        request_id: String,
    },
    AnswerComplete {
        answer: UnderstandingAnswer,
        request_id: String,
    },
    ToolCall {
        trace: ToolTrace,
        request_id: String,
    },
    BudgetLimit {
        reason: String,
        request_id: String,
    },
    Error {
        message: String,
        request_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTrace {
    pub tool: String,
    pub params: serde_json::Value,
    pub returned_bytes: u64,
    pub truncated: bool,
}

// ── Requests ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainRequest {
    pub selection: GraphSelection,
    pub provider_id: String,
    pub explanation_level: String, // "brief" | "detailed"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub session_id: String,
    pub message: String,
    pub provider_id: String,
}
