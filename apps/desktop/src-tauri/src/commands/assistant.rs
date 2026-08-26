// commands/assistant —— explain/chat 流式命令（返工第二轮 B-fix）。
//
// 关键修复：
// - 所有提前 return（`?`、cache hit）都清理 active_requests
// - degraded_answer 带真实 requestId
// - budget.consume 失败终止请求（不再 continue）
// - 六轮工具循环无最终回答 → emit BudgetLimit terminal
// - 每个请求只产生一个 terminal event
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use understanding_gateway::dto::{
    Claim, ClaimClassification, GatewayEvent, GraphSelection, PinnedScope,
};
use understanding_gateway::provider::StreamChunk;
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
    // providerId 为空字符串时视为未指定，回退默认模型（前端选择器默认项）
    let provider_id = payload
        .get("providerId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let level = payload
        .get("explanationLevel")
        .and_then(Value::as_str)
        .unwrap_or("brief")
        .to_string();

    let (snapshot_id, node_id, relation_key) = match &selection {
        GraphSelection::Node {
            snapshot_id,
            node_id,
        } => (snapshot_id.clone(), Some(node_id.clone()), None),
        GraphSelection::Relation {
            snapshot_id,
            relation_key,
            ..
        } => (snapshot_id.clone(), None, Some(relation_key.clone())),
        _ => {
            return Err("explain requires a node or relation selection".to_string());
        }
    };

    // 注册 cancel flag
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    // 辅助：在错误路径上 emit error 并清理 request
    let fail_and_cleanup =
        |app: &tauri::AppHandle, state: &State<'_, AppState>, rid: &str, msg: &str| {
            let _ = app.emit(
                &format!("gateway:{rid}"),
                GatewayEvent::Error {
                    message: msg.to_string(),
                    request_id: rid.to_string(),
                },
            );
            state.active_requests.lock().unwrap().remove(rid);
        };

    // 构建 evidence bundle — 错误路径必须清理
    let index = match common::index_for(&state, &snapshot_id) {
        Ok(i) => i,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &e);
            return Ok(());
        }
    };
    let evidence = if let Some(nid) = &node_id {
        match serde_json::to_value(index.node_context(nid)) {
            Ok(v) => v,
            Err(e) => {
                fail_and_cleanup(
                    &app,
                    &state,
                    &request_id,
                    &format!("evidence build failed: {e}"),
                );
                return Ok(());
            }
        }
    } else if let Some(rk) = &relation_key {
        match index.edge_evidence(rk) {
            Some(b) => match serde_json::to_value(b) {
                Ok(v) => v,
                Err(e) => {
                    fail_and_cleanup(
                        &app,
                        &state,
                        &request_id,
                        &format!("evidence serialize: {e}"),
                    );
                    return Ok(());
                }
            },
            None => {
                fail_and_cleanup(
                    &app,
                    &state,
                    &request_id,
                    &format!("relation not found: {rk}"),
                );
                return Ok(());
            }
        }
    } else {
        unreachable!()
    };
    let (valid_refs, vocab, nodes, relations) = common::evidence_vocabulary(&evidence, &index);

    // 模型配置
    let model = match models::get_model(provider_id.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &format!("model not found: {e}"));
            return Ok(());
        }
    };
    let api_key = match common::resolve_api_key(&state, &model) {
        Ok(k) => k,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &e);
            return Ok(());
        }
    };
    let adapter = match HttpModelAdapter::new(model.clone()) {
        Ok(a) => a,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &format!("adapter init: {e}"));
            return Ok(());
        }
    };

    // cache check — hit 时 emit complete 并清理
    let evidence_json = serde_json::to_string(&evidence)
        .map_err(|e| e.to_string())
        .unwrap_or_default();
    let eh = UnderstandingService::evidence_hash(&evidence_json);
    let cache_key = {
        let gw = state.gateway.lock().unwrap();
        gw.cache_key_for(&eh, &model, "v1", "zh-CN", &level)
    };
    {
        let gw = state.gateway.lock().unwrap();
        if let Some(hit) = gw.cache.get(&cache_key) {
            let answer = hit.clone();
            let _ = app.emit(
                &format!("gateway:{request_id}"),
                GatewayEvent::AnswerComplete {
                    request_id: request_id.clone(),
                    answer: serde_json::from_value(answer).unwrap_or_else(|_| {
                        understanding_gateway::dto::UnderstandingAnswer {
                            schema_version: "codelattice.understandingAnswer.v1".into(),
                            scope: understanding_gateway::dto::AnswerScope {
                                scope_type: common::scope_scope_type(&selection),
                                id: common::scope_id(&selection),
                            },
                            answer_summary: "cache deserialize failed".into(),
                            claims: vec![],
                            navigation_actions: vec![],
                        }
                    }),
                },
            );
            // 返工修复：cache hit 必须清理 active_requests
            state.active_requests.lock().unwrap().remove(&request_id);
            return Ok(());
        }
    }

    let app2 = app.clone();
    let request_id2 = request_id.clone();
    let scope_for_degraded = selection.clone();
    let prompt = {
        let gw = state.gateway.lock().unwrap();
        gw.explain_prompt(&evidence, &level)
    };
    std::thread::spawn(move || {
        // 线程结束时的 deferred cleanup（所有路径都执行）
        let binding = app2.state::<AppState>();
        let rid = request_id2.clone();

        if cancel.load(Ordering::SeqCst) {
            let _ = app2.emit(
                &format!("gateway:{rid}"),
                GatewayEvent::Error {
                    message: "cancelled before start".into(),
                    request_id: rid.clone(),
                },
            );
            binding.active_requests.lock().unwrap().remove(&rid);
            return;
        }

        let result = adapter.stream_explain_with_key(&prompt, api_key.as_deref());
        match result {
            Ok(iter) => {
                let mut text = String::new();
                for chunk in iter {
                    if cancel.load(Ordering::SeqCst) {
                        let _ = app2.emit(
                            &format!("gateway:{rid}"),
                            GatewayEvent::Error {
                                message: "cancelled".into(),
                                request_id: rid.clone(),
                            },
                        );
                        binding.active_requests.lock().unwrap().remove(&rid);
                        return;
                    }
                    match chunk {
                        StreamChunk::Text(t) => {
                            text.push_str(&t);
                            let _ = app2.emit(
                                &format!("gateway:{rid}"),
                                GatewayEvent::AnswerChunk {
                                    text: t,
                                    request_id: rid.clone(),
                                },
                            );
                        }
                        StreamChunk::Done => break,
                    }
                }

                let mut answer = match UnderstandingService::answer_from_model_text(&text) {
                    Ok(mut a) => {
                        UnderstandingService::bind_answer_snapshot(&mut a, &snapshot_id);
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        let gw = binding.gateway.lock().unwrap();
                        let _report =
                            gw.validate_answer(&mut a, &valid_refs, &vocab, &nodes, &relations);
                        a
                    }
                    Err(e) => {
                        // 返工修复：degraded_answer 带真实 requestId
                        if let GatewayEvent::AnswerComplete { mut answer, .. } =
                            common::degraded_answer(
                                &format!("模型输出解析失败：{e}"),
                                scope_for_degraded.clone(),
                                &rid,
                            )
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

                if answer.claims.is_empty() && answer.answer_summary.trim().is_empty() {
                    answer.claims.push(Claim {
                        id: "claim:0".to_string(),
                        text: "模型未返回任何 claim".to_string(),
                        classification: ClaimClassification::Unknown,
                        evidence_refs: vec![],
                        coverage_caveat_refs: vec!["coverage:project:calls".to_string()],
                    });
                }

                let answer_value = serde_json::to_value(&answer).unwrap_or(Value::Null);
                {
                    let mut gw = binding.gateway.lock().unwrap();
                    gw.cache
                        .put(cache_key.clone(), answer_value, common::now_secs());
                }
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::AnswerComplete {
                        request_id: rid.clone(),
                        answer,
                    },
                );
            }
            Err(e) => {
                // 返工修复：degraded_answer 带真实 requestId
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    common::degraded_answer(&e, scope_for_degraded, &rid),
                );
            }
        }
        // 所有路径的最终清理
        binding.active_requests.lock().unwrap().remove(&rid);
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
        .unwrap_or("")
        .to_string();
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if message.trim().is_empty() {
        return Err("empty message".to_string());
    }

    // 返工第二轮：session 必须存在（禁止 adhoc 回退）
    if session_id.is_empty() {
        return Err("sessionId is required".to_string());
    }
    let payload_snapshot_id = payload
        .get("snapshotId")
        .and_then(Value::as_str)
        .ok_or_else(|| "snapshotId is required".to_string())?
        .to_string();
    let pinned_scope: Option<PinnedScope> = serde_json::from_value(
        payload
            .get("pinnedScope")
            .cloned()
            .ok_or_else(|| "pinnedScope is required".to_string())?,
    )
    .map_err(|e| format!("invalid pinnedScope: {e}"))?;

    let snapshot_id = {
        let gw = state.gateway.lock().unwrap();
        gw.sessions
            .validate_turn(&session_id, &payload_snapshot_id, pinned_scope.as_ref())?;
        payload_snapshot_id.clone()
    };

    // providerId 为空字符串时视为未指定，回退默认模型（前端选择器默认项）
    let provider_id = payload
        .get("providerId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let cancel = Arc::new(AtomicBool::new(false));
    state
        .active_requests
        .lock()
        .unwrap()
        .insert(request_id.clone(), cancel.clone());

    let fail_and_cleanup =
        |app: &tauri::AppHandle, state: &State<'_, AppState>, rid: &str, msg: &str| {
            let _ = app.emit(
                &format!("gateway:{rid}"),
                GatewayEvent::Error {
                    message: msg.to_string(),
                    request_id: rid.to_string(),
                },
            );
            state.active_requests.lock().unwrap().remove(rid);
        };

    let index = match common::index_for(&state, &snapshot_id) {
        Ok(i) => i,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &e);
            return Ok(());
        }
    };
    let mut budget = understanding_gateway::dispatcher::BudgetTracker::new();
    let tools = chat_tools_schema();
    let system_prompt = UnderstandingService::chat_system_prompt(&tools);

    let model = match models::get_model(provider_id.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &format!("model: {e}"));
            return Ok(());
        }
    };
    let api_key = match common::resolve_api_key(&state, &model) {
        Ok(k) => k,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &e);
            return Ok(());
        }
    };
    let adapter = match HttpModelAdapter::new(model.clone()) {
        Ok(a) => a,
        Err(e) => {
            fail_and_cleanup(&app, &state, &request_id, &format!("adapter: {e}"));
            return Ok(());
        }
    };

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
    let pinned_scope_text = serde_json::to_string(&pinned_scope).unwrap_or_default();
    std::thread::spawn(move || {
        let binding = app2.state::<AppState>();
        let rid = request_id2.clone();

        if cancel.load(Ordering::SeqCst) {
            let _ = app2.emit(
                &format!("gateway:{rid}"),
                GatewayEvent::Error {
                    message: "cancelled before start".into(),
                    request_id: rid.clone(),
                },
            );
            binding.active_requests.lock().unwrap().remove(&rid);
            return;
        }

        // 构建带 pinned scope 的 context
        let mut context = vec![
            json!({"role": "system", "content": format!("{system_prompt}\n当前对话上下文：pinned scope = {pinned_scope_text}")}),
            json!({"role": "user", "content": message}),
        ];
        context.extend(history);

        let mut evidence_text = String::new();
        let mut got_terminal = false;

        for _round in 0..6u32 {
            if cancel.load(Ordering::SeqCst) {
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::Error {
                        message: "cancelled".into(),
                        request_id: rid.clone(),
                    },
                );
                got_terminal = true;
                break;
            }
            if budget.exhausted() {
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::BudgetLimit {
                        reason: "session tool/evidence budget exhausted (12 calls / 16K tokens)"
                            .to_string(),
                        request_id: rid.clone(),
                    },
                );
                got_terminal = true;
                break;
            }

            let round_prompt = serde_json::to_string(&context).unwrap_or_default();
            let stream = match adapter.stream_explain_with_key(&round_prompt, api_key.as_deref()) {
                Ok(s) => s,
                Err(e) => {
                    let _ = app2.emit(
                        &format!("gateway:{rid}"),
                        GatewayEvent::Error {
                            message: format!("model request failed: {e}"),
                            request_id: rid.clone(),
                        },
                    );
                    got_terminal = true;
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
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::Error {
                        message: "cancelled".into(),
                        request_id: rid.clone(),
                    },
                );
                got_terminal = true;
                break;
            }

            let calls = UnderstandingService::parse_tool_calls(&text);
            if calls.is_empty() {
                // 最终回答
                let (valid_refs, vocab, nodes, relations) =
                    common::evidence_vocabulary(&Value::String(evidence_text.clone()), &index_arc);
                let answer = match UnderstandingService::answer_from_model_text(&text) {
                    Ok(mut a) => {
                        UnderstandingService::bind_answer_snapshot(&mut a, &snapshot_id);
                        for c in &mut a.claims {
                            if c.coverage_caveat_refs.is_empty() {
                                c.coverage_caveat_refs
                                    .push("coverage:project:calls".to_string());
                            }
                        }
                        let gw = binding.gateway.lock().unwrap();
                        let _report =
                            gw.validate_answer(&mut a, &valid_refs, &vocab, &nodes, &relations);
                        a
                    }
                    Err(e) => {
                        if let GatewayEvent::AnswerComplete { answer, .. } = common::degraded_answer(
                            &format!("模型输出解析失败：{e}"),
                            GraphSelection::None,
                            &rid,
                        ) {
                            answer
                        } else {
                            unreachable!()
                        }
                    }
                };
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::AnswerChunk {
                        text: answer.answer_summary.clone(),
                        request_id: rid.clone(),
                    },
                );
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::AnswerComplete {
                        request_id: rid.clone(),
                        answer,
                    },
                );
                {
                    let mut gw = binding.gateway.lock().unwrap();
                    gw.sessions.finish_turn(&session_id2);
                }
                got_terminal = true;
                break;
            }

            // 执行工具
            let mut budget_failed = false;
            for call in calls {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
                // 返工修复：budget 失败 → 终止，不再 continue
                if let Err(budget_err) = budget.consume(&name) {
                    let _ = app2.emit(
                        &format!("gateway:{rid}"),
                        GatewayEvent::BudgetLimit {
                            reason: format!("tool budget exhausted: {budget_err:?}"),
                            request_id: rid.clone(),
                        },
                    );
                    budget_failed = true;
                    got_terminal = true;
                    break;
                }
                let result = index_arc.execute_tool(&name, &arguments);
                let (payload_val, truncated) = match result {
                    Ok(v) => {
                        let s = serde_json::to_string(&v).unwrap_or_default();
                        let (kept, trunc) = understanding_gateway::dispatcher::truncate_bytes(
                            s.as_bytes(),
                            32 * 1024,
                        );
                        (
                            serde_json::from_slice::<Value>(&kept).unwrap_or(Value::Null),
                            trunc,
                        )
                    }
                    Err(e) => (json!({"error": e}), false),
                };
                let payload_str = serde_json::to_string(&payload_val).unwrap_or_default();
                let _ = budget.add_evidence_tokens((payload_str.len() / 4) as u64);
                {
                    let mut gw = binding.gateway.lock().unwrap();
                    gw.sessions.append_trace(
                        &session_id2,
                        understanding_gateway::dto::ToolTrace {
                            tool: name.clone(),
                            params: arguments.clone(),
                            returned_bytes: payload_str.len() as u64,
                            truncated,
                        },
                    );
                }
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::ToolCall {
                        trace: understanding_gateway::dto::ToolTrace {
                            tool: name.clone(),
                            params: arguments.clone(),
                            returned_bytes: payload_str.len() as u64,
                            truncated,
                        },
                        request_id: rid.clone(),
                    },
                );
                evidence_text.push_str(&payload_str);
                context.push(json!({"role": "assistant", "content": format!("tool call: {name}")}));
                context.push(json!({"role": "user", "content": format!("工具 {name} 返回（truncated={truncated}）：\n{payload_str}")}));
            }
            if budget_failed {
                break;
            }

            if cancel.load(Ordering::SeqCst) {
                let _ = app2.emit(
                    &format!("gateway:{rid}"),
                    GatewayEvent::Error {
                        message: "cancelled".into(),
                        request_id: rid.clone(),
                    },
                );
                got_terminal = true;
                break;
            }
        }

        // 返工修复：六轮全是 tool call 且无最终回答 → emit terminal
        if !got_terminal {
            let _ = app2.emit(
                &format!("gateway:{rid}"),
                GatewayEvent::BudgetLimit {
                    reason: "reached max tool rounds (6) without a final answer".to_string(),
                    request_id: rid.clone(),
                },
            );
        }

        binding.active_requests.lock().unwrap().remove(&rid);
    });

    Ok(())
}

#[tauri::command]
pub fn workbench_cancel(state: State<AppState>, request_id: String) -> Result<(), String> {
    if let Some(flag) = state.active_requests.lock().unwrap().get(&request_id) {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    } else {
        Err(format!("no active request: {request_id}"))
    }
}
