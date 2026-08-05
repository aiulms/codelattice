// commands/analyzer —— Desktop Analyzer 命令（返工 G-fix 拆分）。
use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use crate::snapshots;
use crate::AppState;

use super::common;

#[tauri::command]
pub fn workbench_analyze(
    app: tauri::AppHandle,
    state: State<AppState>,
    root: String,
    language: String,
) -> Result<Value, String> {
    let project_root = if root.is_empty() {
        common::repo_root().join("fixtures/rust/portable-smoke")
    } else {
        PathBuf::from(root)
    };
    if !project_root.is_dir() {
        return Err(format!("project root not found: {}", project_root.display()));
    }
    let bin = common::repo_root().join("target/debug/codelattice");
    if !bin.is_file() {
        return Err(format!("codelattice binary not found: {}", bin.display()));
    }
    let publish_dir = snapshots::publish_dir();
    std::fs::create_dir_all(&publish_dir).map_err(|e| e.to_string())?;

    let job_id = state
        .supervisor
        .lock()
        .unwrap()
        .start(project_root, language, bin, publish_dir.clone())?
        .into_job_id();
    let job_id = Some(job_id);

    let app2 = app.clone();
    let publish_dir2 = publish_dir.clone();
    let job_id2 = job_id.clone();
    std::thread::spawn(move || {
        let binding = app2.state::<AppState>();
        let mut sup = binding.supervisor.lock().unwrap();
        match sup.run_to_completion(publish_dir2.clone()) {
            Ok(ev) => {
                let _ = app2.emit("analyzer://event", json!({"jobId": job_id2, "event": format!("{:?}", ev)}));
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
