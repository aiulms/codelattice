// commands —— 薄命令层（P0 §5.2）。
// 每个命令只做参数校验 + 调 Gateway/worker/快照库；业务不在本层膨胀。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};
use understanding_gateway::dto::{
    Claim, ClaimClassification, GatewayEvent, GraphSelection,
};
use understanding_gateway::graph_store::SnapshotGraphIndex;
use understanding_gateway::provider::{ModelAdapter, ModelConfig};
use understanding_gateway::provider_http::HttpModelAdapter;
use understanding_gateway::service::UnderstandingService;

use crate::models;
use crate::snapshots;
use crate::AppState;

fn repo_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../..")
}

// ── Snapshot facts ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn workbench_list_snapshots() -> Result<Value, String> {
    Ok(json!(snapshots::list_snapshots()?))
}

#[tauri::command]
pub fn workbench_load_snapshot(snapshot_id: String) -> Result<Value, String> {
    snapshots::load_snapshot(&snapshot_id)
}

/// G3 选型落地：从已加载 snapshot 构建只读索引并缓存（full immutable graph index）。
fn index_for<'a>(
    state: &'a State<'a, AppState>,
    snapshot_id: &str,
) -> Result<std::sync::Arc<understanding_gateway::graph_store::SnapshotGraphIndex>, String> {
    let mut store = state.query_store.lock().unwrap();
    if let Some(idx) = store.get(snapshot_id) {
        return Ok(idx.clone());
    }
    let data = snapshots::load_snapshot(snapshot_id)?;
    let idx = std::sync::Arc::new(
        understanding_gateway::graph_store::SnapshotGraphIndex::from_snapshot(&data)?,
    );
    store.insert(snapshot_id.to_string(), idx.clone());
    Ok(idx)
}

#[tauri::command]
pub fn workbench_node_context(
    state: State<AppState>,
    snapshot_id: String,
    node_id: String,
) -> Result<Value, String> {
    let idx = index_for(&state, &snapshot_id)?;
    serde_json::to_value(idx.node_context(&node_id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn workbench_edge_evidence(
    state: State<AppState>,
    snapshot_id: String,
    relation_key: String,
    occurrence_key: Option<String>,
) -> Result<Value, String> {
    // occurrenceKey 在 P0 冻结为 relation-level（G2）：请求携带 occurrenceKey
    // 时仍按 relationKey 定位，occurrence 仅作审计保留。
    let idx = index_for(&state, &snapshot_id)?;
    let _ = occurrence_key;
    idx.edge_evidence(&relation_key)
        .map(|bundle| serde_json::to_value(bundle).map_err(|e| e.to_string()))
        .unwrap_or_else(|| Err(format!("relation not found: {relation_key}")))
}

#[tauri::command]
pub fn workbench_call_chain(
    state: State<AppState>,
    snapshot_id: String,
    node_id: String,
    direction: String,
    depth: u32,
) -> Result<Value, String> {
    let idx = index_for(&state, &snapshot_id)?;
    serde_json::to_value(idx.call_chain(&node_id, &direction, depth)).map_err(|e| e.to_string())
}

// ── Model services（P0-B1/B2）──────────────────────────────────────────────

#[tauri::command]
pub fn workbench_models_list() -> Result<Value, String> {
    models::list_models()
}

#[tauri::command]
pub fn workbench_models_add(state: State<AppState>, config: Value) -> Result<Value, String> {
    let cfg: ModelConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    models::add_model(cfg.clone())?;
    let mut gw = state.gateway.lock().unwrap();
    gw.register_model(cfg);
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn workbench_models_remove(state: State<AppState>, id: String) -> Result<Value, String> {
    models::remove_model(&id)?;
    let mut gw = state.gateway.lock().unwrap();
    let (removed, purged) = gw.unregister_model(&id);
    Ok(json!({"ok": removed, "purgedCacheEntries": purged}))
}

#[tauri::command]
pub fn workbench_models_set_default(id: String) -> Result<Value, String> {
    models::set_default(&id)?;
    Ok(json!({"ok": true}))
}

/// 连接测试：不发送任何证据（§7.2）。
#[tauri::command]
pub fn workbench_models_test(state: State<AppState>, id: String) -> Result<Value, String> {
    let model = models::get_model(Some(&id))?;
    // Key 解析（连接测试本身不发送证据；Ollama 无 Key）
    let _api_key = resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model)?;
    let status = adapter.ping();
    Ok(json!({
        "provider": adapter.kind(),
        "ok": status.ok,
        "detail": status.detail,
    }))
}

// ── Secret（G4 gate：Key 零泄漏）──────────────────────────────────────────

#[tauri::command]
pub fn workbench_secret_set(
    state: State<AppState>,
    service: String,
    account: String,
    secret: String,
) -> Result<Value, String> {
    let mut store = state.gateway.lock().unwrap();
    // 借用问题：secret_store 是 Box<dyn SecretStore>，需要从 gateway 取可变引用
    let reference = {
        let store = store.secret_store.as_mut();
        store.set(&service, &account, &secret).map_err(|e| format!("{e:?}"))?
    };
    // 返回值只能是 ref 与掩码（§7.2）
    Ok(json!({
        "secretRef": reference,
        "masked": understanding_gateway::secret::MASKED,
        "stored": true,
    }))
}

#[tauri::command]
pub fn workbench_secret_delete(
    state: State<AppState>,
    secret_ref: String,
) -> Result<Value, String> {
    let mut gw = state.gateway.lock().unwrap();
    let ok = gw
        .secret_store
        .as_mut()
        .delete(&secret_ref)
        .map(|_| true)
        .unwrap_or(false);
    Ok(json!({"ok": ok}))
}

/// 从 SecretStore 解析 Key（仅 Rust 侧使用，绝不回传前端）。
fn resolve_api_key(state: &State<'_, AppState>, model: &ModelConfig) -> Result<Option<String>, String> {
    let Some(reference) = &model.api_key_ref else {
        return Ok(None);
    };
    let gw = state.gateway.lock().unwrap();
    gw.secret_store
        .get(reference)
        .map(Some)
        .map_err(|e| format!("secret {reference} unavailable: {e:?}"))
}

// ── Explain / Chat streaming（P0-B1/B2；channel 桥接）────────────────────

/// 从证据 JSON 收集 valid refs / identifier 词表 / 节点与关系 id（供 validator）。
fn evidence_vocabulary(
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

    // identifier 词表：snapshot 内全部节点 label（bounded 规模可控）
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

/// 构造降级 answer（模型失败/超时/解析失败时，验收 17：事实检查器仍完整可用）。
fn degraded_answer(summary: &str, scope: GraphSelection) -> GatewayEvent {
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

fn scope_scope_type(sel: &GraphSelection) -> understanding_gateway::dto::ConversationScopeType {
    use understanding_gateway::dto::ConversationScopeType;
    match sel {
        GraphSelection::Node { .. } => ConversationScopeType::Node,
        GraphSelection::Relation { .. } => ConversationScopeType::Edge,
        GraphSelection::Chain { .. } => ConversationScopeType::Chain,
        GraphSelection::None => ConversationScopeType::Project,
    }
}

fn scope_id(sel: &GraphSelection) -> String {
    match sel {
        GraphSelection::Node { node_id, .. } => node_id.clone(),
        GraphSelection::Relation { relation_key, .. } => relation_key.clone(),
        GraphSelection::Chain { chain_id, .. } => chain_id.clone(),
        GraphSelection::None => "project".to_string(),
    }
}

#[tauri::command]
pub fn workbench_explain(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    payload: Value,
) -> Result<(), String> {
    let selection: GraphSelection =
        serde_json::from_value(payload.get("selection").cloned().unwrap_or(Value::Null))
            .map_err(|e| format!("invalid selection: {e}"))?;
    let provider_id = payload
        .get("providerId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let level = payload
        .get("explanationLevel")
        .and_then(Value::as_str)
        .unwrap_or("brief")
        .to_string();

    let (snapshot_id, node_id, relation_key) = match &selection {
        GraphSelection::Node { snapshot_id, node_id } => {
            (snapshot_id.clone(), Some(node_id.clone()), None)
        }
        GraphSelection::Relation {
            snapshot_id,
            relation_key,
            ..
        } => (snapshot_id.clone(), None, Some(relation_key.clone())),
        _ => {
            return Err("explain requires a node or relation selection".to_string());
        }
    };

    // 取消注册（P0-B1：UI 取消后立即停止发送事件）
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    // 构建 evidence bundle（G3 full immutable graph index）
    let index = index_for(&state, &snapshot_id)?;
    let evidence = if let Some(nid) = &node_id {
        serde_json::to_value(index.node_context(nid)).map_err(|e| e.to_string())?
    } else if let Some(rk) = &relation_key {
        index
            .edge_evidence(rk)
            .map(|b| serde_json::to_value(b).map_err(|e| e.to_string()))
            .ok_or_else(|| format!("relation not found: {rk}"))??
    } else {
        unreachable!()
    };
    let (valid_refs, vocab, nodes, relations) = evidence_vocabulary(&evidence, &index);

    // 模型配置与 Key（Key 只在本函数栈内存在）
    let model = models::get_model(provider_id.as_deref())?;
    let api_key = resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model.clone())?;

    // 缓存（§6.5：evidenceHash + model + prompt 版本）
    let evidence_json = serde_json::to_string(&evidence).map_err(|e| e.to_string())?;
    let eh = UnderstandingService::evidence_hash(&evidence_json);
    let cache_key = {
        let gw = state.gateway.lock().unwrap();
        gw.cache_key_for(&eh, &model, "v1", "zh-CN", &level)
    };
    if let Some(hit) = state.gateway.lock().unwrap().cache.get(&cache_key) {
        let answer = hit.clone();
        let _ = app.emit(
            &format!("gateway:{request_id}"),
            GatewayEvent::AnswerComplete {
                request_id: request_id.clone(),
                answer: serde_json::from_value(answer).map_err(|e| e.to_string())?,
            },
        );
        return Ok(());
    }

    // 流式线程：请求 → 收集 → 解析 → 校验 → 缓存 → 事件
    let app2 = app.clone();
    let request_id2 = request_id.clone();
    let scope_for_degraded = selection.clone();
    let prompt = {
        let gw = state.gateway.lock().unwrap();
        gw.explain_prompt(&evidence, &level)
    };
    std::thread::spawn(move || {
        let binding = app2.state::<AppState>();
        let mut gw = binding.gateway.lock().unwrap();
        let emit = |ev: GatewayEvent| {
            let _ = app2.emit(&format!("gateway:{request_id2}"), ev);
        };

        if cancel.load(Ordering::SeqCst) {
            emit(GatewayEvent::Error {
                message: "cancelled before start".to_string(),
                request_id: request_id2.clone(),
            });
            binding.active_requests.lock().unwrap().remove(&request_id2);
            return;
        }

        let result = adapter.stream_explain_with_key(&prompt, api_key.as_deref());
        match result {
            Ok(iter) => {
                let mut text = String::new();
                for chunk in iter {
                    if cancel.load(Ordering::SeqCst) {
                        emit(GatewayEvent::Error {
                            message: "cancelled".to_string(),
                            request_id: request_id2.clone(),
                        });
                        binding.active_requests.lock().unwrap().remove(&request_id2);
                        return;
                    }
                    match chunk {
                        understanding_gateway::provider::StreamChunk::Text(t) => {
                            text.push_str(&t);
                            emit(GatewayEvent::AnswerChunk {
                                text: t,
                                request_id: request_id2.clone(),
                            });
                        }
                        understanding_gateway::provider::StreamChunk::Done => break,
                    }
                }

                let mut answer = match UnderstandingService::parse_model_answer(&text) {
                    Ok(mut a) => {
                        // §6.3：coverage caveat 由 Gateway 补充
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        // 校验：claims + navigation（验收 11/12）
                        let _report = gw.validate_answer(&mut a, &valid_refs, &vocab, &nodes, &relations);
                        a
                    }
                    Err(e) => {
                        // 模型输出不可解析 → 降级（验收 17）
                        if let GatewayEvent::AnswerComplete { mut answer, .. } =
                            degraded_answer(&format!("模型输出解析失败：{e}"), scope_for_degraded.clone())
                        {
                            answer.answer_summary = format!(
                                "未能解析模型输出（{e}）。以下为静态降级说明：当前选择没有可用解释。"
                            );
                            answer
                        } else {
                            unreachable!()
                        }
                    }
                };

                // 成功路径补 coverage caveat
                if answer.claims.is_empty() {
                    answer.claims.push(Claim {
                        id: "claim:0".to_string(),
                        text: "模型未返回任何 claim".to_string(),
                        classification: ClaimClassification::Unknown,
                        evidence_refs: vec![],
                        coverage_caveat_refs: vec!["coverage:project:calls".to_string()],
                    });
                }

                let answer_value = serde_json::to_value(&answer).unwrap_or(Value::Null);
                gw.cache.put(cache_key.clone(), answer_value, now_secs());
                emit(GatewayEvent::AnswerComplete {
                    request_id: request_id2.clone(),
                    answer,
                });
            }
            Err(e) => {
                emit(degraded_answer(&e, scope_for_degraded));
            }
        }
        binding.active_requests.lock().unwrap().remove(&request_id2);
    });

    Ok(())
}

/// Chat 只读工具 schema（§6.4 白名单；source_excerpt / change_impact 需确认）。
fn chat_tools_schema() -> Value {
    json!([
        {"name": "project_summary", "description": "项目事实概览（节点/边/CALLS 统计与限制）"},
        {"name": "search_nodes", "parameters": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}}},
        {"name": "get_node_context", "parameters": {"type": "object", "properties": {"nodeId": {"type": "string"}}}},
        {"name": "get_edge_evidence", "parameters": {"type": "object", "properties": {"relationKey": {"type": "string"}}}},
        {"name": "get_call_chain", "parameters": {"type": "object", "properties": {"nodeId": {"type": "string"}, "direction": {"type": "string"}, "depth": {"type": "integer"}}}},
        {"name": "get_static_limitations", "description": "静态分析边界"}
    ])
}

#[tauri::command]
pub fn workbench_chat(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request_id: String,
    payload: Value,
) -> Result<(), String> {
    let session_id = payload
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("sess:adhoc")
        .to_string();
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if message.trim().is_empty() {
        return Err("empty message".to_string());
    }
    let provider_id = payload
        .get("providerId")
        .and_then(Value::as_str)
        .map(str::to_string);
    // 会话 snapshot：Chat pinned 的 snapshot（未 pin 时用当前加载的第一个）
    let snapshot_id = {
        let gw = state.gateway.lock().unwrap();
        gw.sessions
            .get(&session_id)
            .map(|s| s.context.snapshot_id.clone())
            .unwrap_or_default()
    };
    let snapshot_id = if snapshot_id.is_empty() {
        snapshots::list_snapshots()?
            .first()
            .and_then(|m| m.get("id").and_then(Value::as_str))
            .unwrap_or("rust-portable-smoke")
            .to_string()
    } else {
        snapshot_id
    };

    // 取消注册
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    // 工具执行环境：graph index + 预算（默认 L1/L2/L3 冻结值，§5.3）
    let index = index_for(&state, &snapshot_id)?;
    let mut budget = understanding_gateway::dispatcher::BudgetTracker::new();
    let tools = chat_tools_schema();
    let system_prompt = UnderstandingService::chat_system_prompt(&tools);

    // 模型与 Key
    let model = models::get_model(provider_id.as_deref())?;
    let api_key = resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model.clone())?;

    // 会话历史（前 8 轮，防无限增长）
    let history: Vec<Value> = {
        let gw = state.gateway.lock().unwrap();
        gw.sessions
            .get(&session_id)
            .map(|s| {
                s.trace
                    .iter()
                    .filter(|t| t.returned_bytes > 0)
                    .take(8)
                    .map(|t| json!({"role": "assistant", "content": format!("工具 {} 返回了 {} 字节", t.tool, t.returned_bytes)}))
                    .collect()
            })
            .unwrap_or_default()
    };

    let app2 = app.clone();
    let request_id2 = request_id.clone();
    let session_id2 = session_id.clone();
    let index_arc = index.clone();
    std::thread::spawn(move || {
        let binding = app2.state::<AppState>();
        let mut gw = binding.gateway.lock().unwrap();
        let emit = |ev: GatewayEvent| {
            let _ = app2.emit(&format!("gateway:{request_id2}"), ev);
        };

        if cancel.load(Ordering::SeqCst) {
            emit(GatewayEvent::Error {
                message: "cancelled before start".to_string(),
                request_id: request_id2.clone(),
            });
            binding.active_requests.lock().unwrap().remove(&request_id2);
            return;
        }

        // 会话上下文
        let mut context = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": message}),
        ];
        context.extend(history);

        // 工具循环（预算强制执行；max 6 轮防死循环）
        let mut evidence_text = String::new();
        for round in 0..6u32 {
            if cancel.load(Ordering::SeqCst) {
                emit(GatewayEvent::Error {
                    message: "cancelled".to_string(),
                    request_id: request_id2.clone(),
                });
                break;
            }
            if budget.exhausted() {
                emit(GatewayEvent::BudgetLimit {
                    reason: "session tool/evidence budget exhausted (12 calls / 16K tokens)".to_string(),
                    request_id: request_id2.clone(),
                });
                break;
            }

            // 本轮 prompt = 上下文 + 已有工具结果
            let round_prompt = serde_json::to_string(&context).unwrap_or_default();
            let stream = match adapter.stream_explain_with_key(&round_prompt, api_key.as_deref()) {
                Ok(s) => s,
                Err(e) => {
                    emit(GatewayEvent::Error {
                        message: format!("model request failed: {e}"),
                        request_id: request_id2.clone(),
                    });
                    break;
                }
            };
            let mut text = String::new();
            for chunk in stream {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                match chunk {
                    understanding_gateway::provider::StreamChunk::Text(t) => text.push_str(&t),
                    understanding_gateway::provider::StreamChunk::Done => break,
                }
            }
            if cancel.load(Ordering::SeqCst) {
                emit(GatewayEvent::Error {
                    message: "cancelled".to_string(),
                    request_id: request_id2.clone(),
                });
                break;
            }

            // 解析工具调用
            let calls = UnderstandingService::parse_tool_calls(&text);
            if calls.is_empty() {
                // 无工具调用 → 最终回答
                let answer = match UnderstandingService::parse_model_answer(&text) {
                    Ok(mut a) => {
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        a
                    }
                    Err(e) => {
                        if let GatewayEvent::AnswerComplete { answer, .. } =
                            degraded_answer(&format!("模型输出解析失败：{e}"), GraphSelection::None)
                        {
                            answer
                        } else {
                            unreachable!()
                        }
                    }
                };
                emit(GatewayEvent::AnswerChunk {
                    text: answer.answer_summary.clone(),
                    request_id: request_id2.clone(),
                });
                emit(GatewayEvent::AnswerComplete {
                    request_id: request_id2.clone(),
                    answer,
                });
                gw.sessions.complete(&session_id2);
                break;
            }

            // 执行工具调用（逐个；预算强制）
            for call in calls {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let name = call.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
                // 预算：白名单 + 层级配额 + 会话总数
                if let Err(budget_err) = budget.consume(&name) {
                    emit(GatewayEvent::BudgetLimit {
                        reason: format!("tool rejected: {budget_err:?}"),
                        request_id: request_id2.clone(),
                    });
                    continue;
                }
                // 执行（graph store 只读工具）
                let result = index_arc.execute_tool(&name, &arguments);
                let (payload_val, truncated) = match result {
                    Ok(v) => {
                        let s = serde_json::to_string(&v).unwrap_or_default();
                        let (kept, trunc) =
                            understanding_gateway::dispatcher::truncate_bytes(s.as_bytes(), 32 * 1024);
                        (
                            serde_json::from_slice::<Value>(&kept).unwrap_or(Value::Null),
                            trunc,
                        )
                    }
                    Err(e) => (json!({"error": e}), false),
                };
                let payload_str = serde_json::to_string(&payload_val).unwrap_or_default();
                let _ = budget.add_evidence_tokens((payload_str.len() / 4) as u64);
                // trace 记录（可展开 session trace，验收 15）
                gw.sessions.append_trace(
                    &session_id2,
                    understanding_gateway::dto::ToolTrace {
                        tool: name.clone(),
                        params: arguments.clone(),
                        returned_bytes: payload_str.len() as u64,
                        truncated,
                    },
                );
                emit(GatewayEvent::ToolCall {
                    trace: understanding_gateway::dto::ToolTrace {
                        tool: name.clone(),
                        params: arguments.clone(),
                        returned_bytes: payload_str.len() as u64,
                        truncated,
                    },
                    request_id: request_id2.clone(),
                });
                evidence_text.push_str(&payload_str);
                context.push(json!({
                    "role": "assistant",
                    "content": format!("tool call: {name}"),
                }));
                context.push(json!({
                    "role": "user",
                    "content": format!("工具 {name} 返回（可能已截断，truncated={truncated}）：\n{payload_str}"),
                }));
            }
            let _ = round;
        }
        binding.active_requests.lock().unwrap().remove(&request_id2);
    });

    Ok(())
}

#[tauri::command]
pub fn workbench_cancel(
    state: State<AppState>,
    request_id: String,
) -> Result<(), String> {
    if let Some(flag) = state.active_requests.lock().unwrap().get(&request_id) {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    } else {
        Err(format!("no active request: {request_id}"))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Selftest / smoke（G1 gate）──────────────────────────────────────────────

#[tauri::command]
pub fn workbench_selftest_enabled() -> bool {
    std::env::var("CODELATTICE_SELFTEST").map(|v| v == "1").unwrap_or(false)
}

#[tauri::command]
pub fn workbench_smoke_report(payload: Value) -> Result<(), String> {
    let out = std::env::var("CODELATTICE_SMOKE_OUT").unwrap_or_else(|_| {
        repo_root().join("target/selftest-report.json").to_string_lossy().to_string()
    });
    let path = PathBuf::from(&out);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ── Desktop Analyzer（F1 #5 / P0-C）─────────────────────────────────────────

#[tauri::command]
pub fn workbench_analyze(
    app: tauri::AppHandle,
    state: State<AppState>,
    root: String,
    language: String,
) -> Result<Value, String> {
    let project_root = if root.is_empty() {
        repo_root().join("fixtures/rust/portable-smoke")
    } else {
        PathBuf::from(root)
    };
    if !project_root.is_dir() {
        return Err(format!("project root not found: {}", project_root.display()));
    }
    let bin = repo_root().join("target/debug/codelattice");
    if !bin.is_file() {
        return Err(format!("codelattice binary not found: {}", bin.display()));
    }
    // P0-C：Desktop Analyzer 只发布到独立 publish 目录（不触碰 fixtures 基线）
    let publish_dir = snapshots::publish_dir();
    std::fs::create_dir_all(&publish_dir).map_err(|e| e.to_string())?;

    let job_id = state
        .supervisor
        .lock()
        .unwrap()
        .start(project_root, language, bin, publish_dir.clone())?
        .into_job_id();
    let job_id = Some(job_id);

    // 后台线程：等待完成 → atomic publish → 事件 + cleanup（P0-C）
    let app2 = app.clone();
    let publish_dir2 = publish_dir.clone();
    let job_id2 = job_id.clone();
    std::thread::spawn(move || {
        use tauri::Emitter;
        let binding = app2.state::<AppState>();
        let mut sup = binding.supervisor.lock().unwrap();
        match sup.run_to_completion(publish_dir2.clone()) {
            Ok(ev) => {
                let _ = app2.emit("analyzer://event", json!({"jobId": job_id2, "event": format!("{:?}", ev)}));
                // 完成后清理：保留 pinned + 最近 2 个（只清理发布目录）
                let pinned = binding.pinned_snapshots.lock().unwrap().clone();
                let _ = snapshots::cleanup_published(2, &pinned);
            }
            Err(e) => {
                let _ = app2.emit("analyzer://event", json!({"jobId": job_id2, "error": e}));
            }
        }
    });

    Ok(json!({"jobId": job_id, "started": true}))
}

#[tauri::command]
pub fn workbench_pin_snapshot(state: State<AppState>, snapshot_id: String) -> Result<Value, String> {
    // 验证存在后再 pin
    snapshots::load_snapshot(&snapshot_id)?;
    let mut pinned = state.pinned_snapshots.lock().unwrap();
    if !pinned.contains(&snapshot_id) {
        pinned.push(snapshot_id);
    }
    Ok(json!({"pinned": pinned.clone()}))
}

#[tauri::command]
pub fn workbench_unpin_snapshot(state: State<AppState>, snapshot_id: String) -> Result<Value, String> {
    let mut pinned = state.pinned_snapshots.lock().unwrap();
    pinned.retain(|id| id != &snapshot_id);
    Ok(json!({"pinned": pinned.clone()}))
}

#[tauri::command]
pub fn workbench_analyze_cancel(state: State<AppState>) -> Result<(), String> {
    state.supervisor.lock().unwrap().request_cancel();
    Ok(())
}

#[tauri::command]
pub fn workbench_analyze_status(state: State<AppState>) -> Result<Value, String> {
    let sup = state.supervisor.lock().unwrap();
    Ok(json!({
        "state": format!("{:?}", sup.state()),
        "jobId": sup.active_job_id(),
    }))
}
