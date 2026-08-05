// commands/analyzer —— Desktop Analyzer 命令（返工第二轮 C-fix）。
//
// 关键修复：
// - 不在 wait 期间持有 supervisor mutex
// - status 返回 publishedSnapshotId
// - cancel 在运行中 kill child
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
        return Err(format!(
            "project root not found: {}",
            project_root.display()
        ));
    }
    let bin = common::repo_root().join("target/debug/codelattice");
    if !bin.is_file() {
        return Err(format!("codelattice binary not found: {}", bin.display()));
    }
    let publish_dir = snapshots::publish_dir();
    std::fs::create_dir_all(&publish_dir).map_err(|e| e.to_string())?;

    // start 只做短操作（spawn + insert）
    let job_id = {
        state
            .supervisor
            .start(project_root, language, bin, publish_dir.clone())?
    };

    // 后台线程：锁外 wait，完成后 emit
    let app2 = app.clone();
    std::thread::spawn(move || {
        let binding = app2.state::<AppState>();
        let result = binding.supervisor.wait_and_publish();

        // emit 结果
        let _ = app2.emit(
            "analyzer://event",
            json!({
                "jobId": result.job_id,
                "state": format!("{:?}", result.state),
                "publishedSnapshotId": result.published_snapshot_id,
                "error": result.error,
            }),
        );

        // cleanup published（保留 pinned + 最近 2）
        if result.state == crate::analyzer::AnalyzerState::Completed {
            let pinned = binding.pinned_snapshots.lock().unwrap().clone();
            let _ = snapshots::cleanup_published(2, &pinned);
        }
    });

    Ok(json!({"jobId": job_id, "started": true}))
}

#[tauri::command]
pub fn workbench_pin_snapshot(
    state: State<AppState>,
    snapshot_id: String,
) -> Result<Value, String> {
    snapshots::load_snapshot(&snapshot_id)?;
    let mut pinned = state.pinned_snapshots.lock().unwrap();
    if !pinned.contains(&snapshot_id) {
        pinned.push(snapshot_id.clone());
    }
    // 同步更新 query store pin 状态
    state
        .query_store
        .lock()
        .unwrap()
        .set_pinned(&snapshot_id, true);
    Ok(json!({"pinned": pinned.clone()}))
}

#[tauri::command]
pub fn workbench_unpin_snapshot(
    state: State<AppState>,
    snapshot_id: String,
) -> Result<Value, String> {
    let mut pinned = state.pinned_snapshots.lock().unwrap();
    pinned.retain(|id| id != &snapshot_id);
    state
        .query_store
        .lock()
        .unwrap()
        .set_pinned(&snapshot_id, false);
    Ok(json!({"pinned": pinned.clone()}))
}

#[tauri::command]
pub fn workbench_analyze_cancel(state: State<AppState>) -> Result<(), String> {
    state.supervisor.request_cancel();
    Ok(())
}

#[tauri::command]
pub fn workbench_analyze_status(state: State<AppState>) -> Result<Value, String> {
    let analyzer_state = state.supervisor.analyzer_state();
    let result = state.supervisor.last_result();
    Ok(json!({
        "state": format!("{:?}", analyzer_state),
        "jobId": state.supervisor.active_job_id(),
        "publishedSnapshotId": result.as_ref().and_then(|r| r.published_snapshot_id.clone()),
        "error": result.as_ref().and_then(|r| r.error.clone()),
    }))
}
