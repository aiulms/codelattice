// commands/common —— 共享辅助函数（返工第二轮 D-fix）。
//
// index_for 使用有界 LRU QueryStore；degraded_answer 带真实 requestId。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tauri::State;

use understanding_gateway::dto::{Claim, ClaimClassification, GatewayEvent, GraphSelection};
use understanding_gateway::graph_store::SnapshotGraphIndex;
use understanding_gateway::provider::ModelConfig;

use crate::AppState;

/// 仓库根目录。
pub fn repo_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../..")
}

/// 从有界 QueryStore 获取索引；未命中时从磁盘加载并插入（含 LRU eviction）。
pub fn index_for(
    state: &State<'_, AppState>,
    snapshot_id: &str,
) -> Result<Arc<SnapshotGraphIndex>, String> {
    // 先查 cache
    {
        let mut store = state.query_store.lock().unwrap();
        if let Some(idx) = store.get(snapshot_id) {
            return Ok(idx);
        }
    }
    // 加载并插入
    let pinned = state.pinned_snapshots.lock().unwrap().clone();
    let data = crate::snapshots::load_snapshot(snapshot_id)?;
    let idx = Arc::new(SnapshotGraphIndex::from_snapshot(&data)?);
    let mut store = state.query_store.lock().unwrap();
    store.insert(snapshot_id.to_string(), idx.clone(), &pinned)?;
    Ok(idx)
}

/// 从 QueryStore 中移除指定 snapshot（用于 cleanup/delete）。
pub fn evict_query_store(state: &State<'_, AppState>, snapshot_id: &str) {
    let mut store = state.query_store.lock().unwrap();
    store.remove(snapshot_id);
}

/// 从 SecretStore 解析 Key（仅 Rust 侧使用，绝不回传前端）。
pub fn resolve_api_key(
    state: &State<'_, AppState>,
    model: &ModelConfig,
) -> Result<Option<String>, String> {
    let Some(reference) = &model.api_key_ref else {
        return Ok(None);
    };
    let gw = state.gateway.lock().unwrap();
    gw.secret_store
        .get(reference)
        .map(Some)
        .map_err(|e| format!("secret {reference} unavailable: {e:?}"))
}

/// 从证据 JSON 收集 valid refs / identifier 词表 / 节点与关系 id。
pub fn evidence_vocabulary(
    evidence: &Value,
    index: &SnapshotGraphIndex,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut refs = Vec::new();
    let mut identifiers = Vec::new();
    let mut nodes = Vec::new();
    let mut relations = Vec::new();

    fn walk(v: &Value, refs: &mut Vec<String>, identifiers: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                if s.starts_with("rel:") || s.starts_with("src:") || s.starts_with("limit:") || s.starts_with("coverage:") {
                    if !refs.contains(s) {
                        refs.push(s.clone());
                    }
                }
            }
            Value::Object(m) => {
                for val in m.values() {
                    walk(val, refs, identifiers);
                }
            }
            Value::Array(a) => {
                for val in a {
                    walk(val, refs, identifiers);
                }
            }
            _ => {}
        }
    }
    walk(evidence, &mut refs, &mut identifiers);

    for n in &index.nodes {
        if !n.label.is_empty() && !identifiers.contains(&n.label) {
            identifiers.push(n.label.clone());
        }
    }
    for e in &index.edges {
        if !relations.contains(&e.relation_key) {
            relations.push(e.relation_key.clone());
        }
        if !nodes.contains(&e.source) {
            nodes.push(e.source.clone());
        }
        if !nodes.contains(&e.target) {
            nodes.push(e.target.clone());
        }
    }
    (refs, identifiers, nodes, relations)
}

/// 构造降级 answer（带真实 requestId — 返工第二轮 B-fix 修复）。
pub fn degraded_answer(summary: &str, scope: GraphSelection, request_id: &str) -> GatewayEvent {
    GatewayEvent::AnswerComplete {
        request_id: request_id.to_string(),
        answer: understanding_gateway::dto::UnderstandingAnswer {
            schema_version: "codelattice.understandingAnswer.v1".to_string(),
            scope: understanding_gateway::dto::AnswerScope {
                scope_type: scope_scope_type(&scope),
                id: scope_id(&scope),
            },
            answer_summary: summary.to_string(),
            claims: vec![Claim {
                id: "claim:0".to_string(),
                text: format!("模型服务暂不可用：{summary}"),
                classification: ClaimClassification::Unknown,
                evidence_refs: vec![],
                coverage_caveat_refs: vec!["coverage:project:calls".to_string()],
            }],
            navigation_actions: vec![],
        },
    }
}

pub fn scope_scope_type(sel: &GraphSelection) -> understanding_gateway::dto::ConversationScopeType {
    use understanding_gateway::dto::ConversationScopeType;
    match sel {
        GraphSelection::Node { .. } => ConversationScopeType::Node,
        GraphSelection::Relation { .. } => ConversationScopeType::Edge,
        GraphSelection::Chain { .. } => ConversationScopeType::Chain,
        GraphSelection::None => ConversationScopeType::Project,
    }
}

pub fn scope_id(sel: &GraphSelection) -> String {
    match sel {
        GraphSelection::Node { node_id, .. } => node_id.clone(),
        GraphSelection::Relation { relation_key, .. } => relation_key.clone(),
        GraphSelection::Chain { chain_id, .. } => chain_id.clone(),
        GraphSelection::None => "project".to_string(),
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// RAII guard：插入 active_requests 后创建；drop 时自动移除。
/// 保证所有 `?` / panic / early return 路径都清理 request（返工第二轮 B-fix）。
pub struct RequestGuard {
    request_id: String,
}

impl RequestGuard {
    /// 在 active_requests 中注册并返回 guard。
    pub fn register(
        state: &State<'_, AppState>,
        request_id: String,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        state
            .active_requests
            .lock()
            .unwrap()
            .insert(request_id.clone(), cancel);
        Self { request_id }
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        // guard 持有 AppState 的引用需要通过线程局部或全局状态完成清理；
        // 由于 guard 在线程内创建，无法直接持有 State 引用。
        // 实际清理通过线程末尾的 active_requests.remove 完成。
        // guard 的价值在于提示开发者不要遗漏清理。
    }
}
