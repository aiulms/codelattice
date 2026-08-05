// commands/sessions —— 会话生命周期命令（返工新增 G-fix 拆分）。
use tauri::State;

use crate::AppState;
use crate::snapshots;

use super::common;

#[tauri::command]
pub fn workbench_session_create(state: State<AppState>, snapshot_id: String) -> Result<String, String> {
    snapshots::load_snapshot(&snapshot_id)?;
    let mut gw = state.gateway.lock().unwrap();
    let now = common::now_secs();
    let id = gw.sessions.create(&snapshot_id, now);
    Ok(id)
}

#[tauri::command]
pub fn workbench_session_pin(
    state: State<AppState>,
    session_id: String,
    scope_type: String,
    scope_id: String,
    snapshot_id: String,
) -> Result<(), String> {
    let scope = match scope_type.as_str() {
        "node" => understanding_gateway::dto::ConversationScopeType::Node,
        "edge" => understanding_gateway::dto::ConversationScopeType::Edge,
        "chain" => understanding_gateway::dto::ConversationScopeType::Chain,
        _ => understanding_gateway::dto::ConversationScopeType::Project,
    };
    let mut gw = state.gateway.lock().unwrap();
    if !gw.sessions.pin(&session_id, scope, &scope_id, &snapshot_id) {
        return Err(format!("session not found: {session_id}"));
    }
    Ok(())
}

#[tauri::command]
pub fn workbench_session_close(state: State<AppState>, session_id: String) -> Result<(), String> {
    let mut gw = state.gateway.lock().unwrap();
    if !gw.sessions.close(&session_id) {
        return Err(format!("session not found: {session_id}"));
    }
    Ok(())
}

#[tauri::command]
pub fn workbench_select_directory() -> Result<String, String> {
    if let Ok(root) = std::env::var("CODELATTICE_ANALYZE_ROOT") {
        return Ok(root);
    }
    Ok(common::repo_root().join("fixtures/rust/portable-smoke").to_string_lossy().to_string())
}
