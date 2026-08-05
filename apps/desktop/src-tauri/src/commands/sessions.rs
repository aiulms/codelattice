// commands/sessions —— 会话生命周期命令（返工新增 G-fix 拆分）。
use tauri::State;

use crate::snapshots;
use crate::AppState;

use super::common;

fn parse_scope_type(
    value: &str,
) -> Result<understanding_gateway::dto::ConversationScopeType, String> {
    use understanding_gateway::dto::ConversationScopeType;
    match value {
        "project" => Ok(ConversationScopeType::Project),
        "node" => Ok(ConversationScopeType::Node),
        "edge" => Ok(ConversationScopeType::Edge),
        "chain" => Ok(ConversationScopeType::Chain),
        _ => Err(format!("invalid conversation scope type: {value}")),
    }
}

#[tauri::command]
pub fn workbench_session_create(
    state: State<AppState>,
    snapshot_id: String,
) -> Result<String, String> {
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
    let scope = parse_scope_type(&scope_type)?;
    let mut gw = state.gateway.lock().unwrap();
    if !gw.sessions.pin(&session_id, scope, &scope_id, &snapshot_id) {
        return Err(format!("session not found: {session_id}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_scope_type_is_rejected_instead_of_silently_becoming_project() {
        assert!(parse_scope_type("typo").is_err());
        assert!(parse_scope_type("project").is_ok());
    }
}

#[tauri::command]
pub fn workbench_session_close(state: State<AppState>, session_id: String) -> Result<(), String> {
    let mut gw = state.gateway.lock().unwrap();
    if !gw.sessions.close(&session_id) {
        return Err(format!("session not found: {session_id}"));
    }
    Ok(())
}
