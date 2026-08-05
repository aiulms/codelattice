// commands/common —— 拆分后的共享辅助函数与 query_store 管理（返工 G-fix）。
//
// 所有需要 index_for / resolve_api_key / evidence_vocabulary 的子模块引用本文件。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tauri::State;

use understanding_gateway::dto::{Claim, ClaimClassification, GatewayEvent, GraphSelection};
use understanding_gateway::graph_store::SnapshotGraphIndex;
use understanding_gateway::provider::ModelConfig;

use crate::AppState;

/// 仓库根目录（src-tauri 的 CARGO_MANIFEST_DIR 向上三级）。
pub fn repo_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../..")
}

/// 返工修复：聚合 query_store 内存上限。
pub const MAX_QUERY_STORE_SNAPSHOTS: usize = 8;

/// 从已加载 snapshot 构建只读索引并缓存；超过上限时按 LRU 逐出（pinned 保留）。
pub fn index_for(
    state: &State<'_, AppState>,
    snapshot_id: &str,
) -> Result<Arc<SnapshotGraphIndex>, String> {
    let mut store = state.query_store.lock().unwrap();
    if let Some(idx) = store.get(snapshot_id) {
        return Ok(idx.clone());
    }

    let pinned = state.pinned_snapshots.lock().unwrap();
    if store.len() >= MAX_QUERY_STORE_SNAPSHOTS {
        let to_remove: Option<String> = store.keys().find(|k| !pinned.contains(k)).cloned();
        drop(pinned);
        if let Some(key) = to_remove {
            store.remove(&key);
        }
    }

    let data = crate::snapshots::load_snapshot(snapshot_id)?;
    let idx = Arc::new(SnapshotGraphIndex::from_snapshot(&data)?);
    store.insert(snapshot_id.to_string(), idx.clone());
    Ok(idx)
}

/// 从 query_store 中移除指定 snapshot 的内存索引。
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

/// 从证据 JSON 收集 valid refs / identifier 词表 / 节点与关系 id（供 validator）。
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

/// 构造降级 answer（模型失败/超时/解析失败时，事实检查器仍完整可用）。
pub fn degraded_answer(summary: &str, scope: GraphSelection) -> GatewayEvent {
    GatewayEvent::AnswerComplete {
        request_id: String::new(),
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
