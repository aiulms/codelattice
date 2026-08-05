// commands/evidence —— snapshot 查询命令（返工 G-fix 拆分）。
use serde_json::{json, Value};
use tauri::State;

use crate::AppState;
use super::common;

#[tauri::command]
pub fn workbench_list_snapshots() -> Result<Value, String> {
    Ok(json!(crate::snapshots::list_snapshots()?))
}

#[tauri::command]
pub fn workbench_load_snapshot(snapshot_id: String) -> Result<Value, String> {
    crate::snapshots::load_snapshot(&snapshot_id)
}

#[tauri::command]
pub fn workbench_node_context(
    state: State<AppState>,
    snapshot_id: String,
    node_id: String,
) -> Result<Value, String> {
    let idx = common::index_for(&state, &snapshot_id)?;
    serde_json::to_value(idx.node_context(&node_id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn workbench_edge_evidence(
    state: State<AppState>,
    snapshot_id: String,
    relation_key: String,
    occurrence_key: Option<String>,
) -> Result<Value, String> {
    let idx = common::index_for(&state, &snapshot_id)?;
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
    let idx = common::index_for(&state, &snapshot_id)?;
    serde_json::to_value(idx.call_chain(&node_id, &direction, depth)).map_err(|e| e.to_string())
}
