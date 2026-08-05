// commands/selftest —— selftest/smoke 命令（返工 G-fix 拆分）。
use std::path::PathBuf;

use serde_json::Value;
use tauri::State;

use crate::AppState;

use super::common;

#[tauri::command]
pub fn workbench_selftest_enabled() -> bool {
    std::env::var("CODELATTICE_SELFTEST").map(|v| v == "1").unwrap_or(false)
}

#[tauri::command]
pub fn workbench_smoke_report(payload: Value) -> Result<(), String> {
    let out = std::env::var("CODELATTICE_SMOKE_OUT").unwrap_or_else(|_| {
        common::repo_root().join("target/selftest-report.json").to_string_lossy().to_string()
    });
    let path = PathBuf::from(&out);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
