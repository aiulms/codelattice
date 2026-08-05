// commands/assistant —— explain/chat 流式命令（返工 G-fix 拆分）。
//
// 包含 explain、chat、cancel 及辅助函数。
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use understanding_gateway::dto::{Claim, ClaimClassification, GatewayEvent, GraphSelection};
use understanding_gateway::provider::{ModelAdapter, StreamChunk};
use understanding_gateway::provider_http::HttpModelAdapter;
use understanding_gateway::service::UnderstandingService;

use crate::models;
use crate::AppState;

use super::common;

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

    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    let index = common::index_for(&state, &snapshot_id)?;
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
    let (valid_refs, vocab, nodes, relations) = common::evidence_vocabulary(&evidence, &index);

    let model = models::get_model(provider_id.as_deref())?;
    let api_key = common::resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model.clone())?;

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
                        StreamChunk::Text(t) => {
                            text.push_str(&t);
                            emit(GatewayEvent::AnswerChunk {
                                text: t,
                                request_id: request_id2.clone(),
                            });
                        }
                        StreamChunk::Done => break,
                    }
                }

                let mut answer = match UnderstandingService::parse_model_answer(&text) {
                    Ok(mut a) => {
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        let _report = gw.validate_answer(&mut a, &valid_refs, &vocab, &nodes, &relations);
                        a
                    }
                    Err(e) => {
                        if let GatewayEvent::AnswerComplete { mut answer, .. } =
                            common::degraded_answer(&format!("模型输出解析失败：{e}"), scope_for_degraded.clone())
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
                gw.cache.put(cache_key.clone(), answer_value, common::now_secs());
                emit(GatewayEvent::AnswerComplete {
                    request_id: request_id2.clone(),
                    answer,
                });
            }
            Err(e) => {
                emit(common::degraded_answer(&e, scope_for_degraded));
            }
        }
        binding.active_requests.lock().unwrap().remove(&request_id2);
    });

    Ok(())
}

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

    // 返工修复：snapshot 来自前端或 session；未知 session 返回错误
    let snapshot_id = {
        let from_payload = payload
            .get("snapshotId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let gw = state.gateway.lock().unwrap();
        if let Some(sid) = from_payload {
            if !session_id.is_empty() && session_id != "sess:adhoc" {
                if !gw.sessions.exists(&session_id) {
                    return Err(format!("session not found: {session_id}"));
                }
            }
            sid
        } else if let Some(s) = gw.sessions.get(&session_id) {
            s.context.snapshot_id.clone()
        } else if session_id.is_empty() || session_id == "sess:adhoc" {
            drop(gw);
            crate::snapshots::list_snapshots()?
                .first()
                .and_then(|m| m.get("id").and_then(Value::as_str))
                .unwrap_or("rust-portable-smoke")
                .to_string()
        } else {
            return Err(format!("session not found: {session_id}"));
        }
    };

    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    let index = common::index_for(&state, &snapshot_id)?;
    let mut budget = understanding_gateway::dispatcher::BudgetTracker::new();
    let tools = chat_tools_schema();
    let system_prompt = UnderstandingService::chat_system_prompt(&tools);

    let model = models::get_model(provider_id.as_deref())?;
    let api_key = common::resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model.clone())?;

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

        let mut context = vec![
            json!({"role": "system", "content": system_prompt}),
            json!({"role": "user", "content": message}),
        ];
        context.extend(history);

        let mut evidence_text = String::new();
        for round in 0..6u32 {
            if cancel.load(Ordering::SeqCst) {
                emit(GatewayEvent::Error {
                    message: "cancelled".to_string(),
                    request_id: request_id2.clone(),
                });
                binding.active_requests.lock().unwrap().remove(&request_id2);
                return;
            }
            if budget.exhausted() {
                emit(GatewayEvent::BudgetLimit {
                    reason: "session tool/evidence budget exhausted (12 calls / 16K tokens)".to_string(),
                    request_id: request_id2.clone(),
                });
                break;
            }

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
                    StreamChunk::Text(t) => text.push_str(&t),
                    StreamChunk::Done => break,
                }
            }
            if cancel.load(Ordering::SeqCst) {
                emit(GatewayEvent::Error {
                    message: "cancelled".to_string(),
                    request_id: request_id2.clone(),
                });
                binding.active_requests.lock().unwrap().remove(&request_id2);
                return;
            }

            let calls = UnderstandingService::parse_tool_calls(&text);
            if calls.is_empty() {
                // 最终回答：走 validate_answer（返工修复）
                let (valid_refs, vocab, nodes, relations) = common::evidence_vocabulary(
                    &Value::String(evidence_text.clone()),
                    &index_arc,
                );
                let answer = match UnderstandingService::parse_model_answer(&text) {
                    Ok(mut a) => {
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        let _report = gw.validate_answer(&mut a, &valid_refs, &vocab, &nodes, &relations);
                        a
                    }
                    Err(e) => {
                        if let GatewayEvent::AnswerComplete { answer, .. } =
                            common::degraded_answer(&format!("模型输出解析失败：{e}"), GraphSelection::None)
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

            for call in calls {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let name = call.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
                if let Err(budget_err) = budget.consume(&name) {
                    emit(GatewayEvent::BudgetLimit {
                        reason: format!("tool rejected: {budget_err:?}"),
                        request_id: request_id2.clone(),
                    });
                    continue;
                }
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

            if cancel.load(Ordering::SeqCst) {
                emit(GatewayEvent::Error {
                    message: "cancelled".to_string(),
                    request_id: request_id2.clone(),
                });
                binding.active_requests.lock().unwrap().remove(&request_id2);
                return;
            }
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
