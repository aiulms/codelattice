// commands/selftest —— selftest/smoke 命令（返工 G-fix 拆分 / I-fix 端口检测）。
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};
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

/// I-fix: 检测 Vite dev server 端口可达性。
///
/// 在 selftest 启动前由前端调用，确认 1420 端口已就绪。
/// 如果端口未就绪，selftest 应等待而非立即失败。
#[tauri::command]
pub fn workbench_check_port(port: Option<u16>) -> Result<Value, String> {
    let port = port.unwrap_or(1420);
    let addr = format!("127.0.0.1:{}", port);

    // 尝试 TCP 连接（2 秒超时）
    let reachable = std::net::TcpStream::connect_timeout(
        &addr.parse::<std::net::SocketAddr>().map_err(|e| e.to_string())?,
        Duration::from_secs(2),
    ).is_ok();

    Ok(json!({
        "port": port,
        "reachable": reachable,
        "addr": addr,
    }))
}
