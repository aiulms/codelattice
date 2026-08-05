// commands/selftest —— selftest/smoke 命令（返工 G-fix 拆分 / I-fix 端口检测）。
use serde_json::Value;
use std::path::PathBuf;
use std::{fs::OpenOptions, io::Write};
use tauri::AppHandle;

use super::common;

/// 仅 selftest 环境写阶段标记，定位 production WebView/IPC 启动故障。
pub fn trace(stage: &str) {
    if std::env::var("CODELATTICE_SELFTEST").as_deref() != Ok("1") {
        return;
    }
    let out = std::env::var("CODELATTICE_SMOKE_OUT").unwrap_or_else(|_| {
        common::repo_root()
            .join("target/selftest-report.json")
            .to_string_lossy()
            .to_string()
    });
    let path = PathBuf::from(out).with_extension("trace.log");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{stage}");
    }
}

#[tauri::command]
pub fn workbench_selftest_enabled() -> bool {
    trace("ipc:selftest-enabled");
    std::env::var("CODELATTICE_SELFTEST")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[tauri::command]
pub fn workbench_selftest_probe(details: Value) {
    trace(&format!("webview:probe:{details}"));
}

#[tauri::command]
pub fn workbench_smoke_report(app: AppHandle, payload: Value) -> Result<(), String> {
    trace("ipc:smoke-report");
    let out = std::env::var("CODELATTICE_SMOKE_OUT").unwrap_or_else(|_| {
        common::repo_root()
            .join("target/selftest-report.json")
            .to_string_lossy()
            .to_string()
    });
    let path = PathBuf::from(&out);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if std::env::var("CODELATTICE_SELFTEST_EXIT").as_deref() == Ok("1") {
        // 给采样器留出最后一个时间片；只退出本次精确启动的应用进程。
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            app.exit(0);
        });
    }
    Ok(())
}
