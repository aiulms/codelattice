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

/// 文件夹体检（多语言卡 3）：同步调 CLI `inspect`，毫秒级扫描——
/// 不套 analyze 的 `nice -n 10`，不进 AnalyzerSupervisor、不占 analyze
/// 单任务 gate、不可取消。
#[tauri::command]
pub fn workbench_inspect(root: String) -> Result<Value, String> {
    let bin = common::repo_root().join("target/debug/codelattice");
    inspect_with_cli(&bin, &root)
}

/// 校验通过后**原样透传** envelope，不在 Rust 侧重建模字段——
/// 契约单一事实源在 core CLI（analyzable 也由 CLI 算好，桌面禁止再做 feature 判定）。
fn inspect_with_cli(bin: &std::path::Path, root: &str) -> Result<Value, String> {
    if !bin.is_file() {
        return Err(format!("codelattice binary not found: {}", bin.display()));
    }
    let output = std::process::Command::new(bin)
        .args(["inspect", "--root", root, "--format", "json"])
        .output()
        .map_err(|e| format!("spawn inspect failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "inspect failed: {}",
            stderr.trim().chars().take(300).collect::<String>()
        ));
    }
    let envelope: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("inspect stdout is not valid JSON: {e}"))?;
    // 发布前守卫风格：schemaVersion 必须逐字等于 v1，不认识就点名实际值拒绝
    if envelope["schemaVersion"] != "codelattice.workspaceInspection.v1" {
        return Err(format!(
            "unexpected inspect schemaVersion: {} (expected codelattice.workspaceInspection.v1)",
            envelope["schemaVersion"]
        ));
    }
    Ok(envelope)
}

#[cfg(test)]
mod inspect_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "codelattice-inspect-cmd-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 假 CLI：忽略参数，把给定 JSON 原文吐到 stdout。
    /// 与 analyzer.rs 测试的 fake_cli 同模式，按执行卡不抽共享。
    fn fake_cli(dir: &std::path::Path, label: &str, body: &str) -> PathBuf {
        let script = dir.join(format!("{label}.sh"));
        fs::write(&script, format!("#!/bin/sh\nprintf '%s' '{body}'\n")).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        script
    }

    #[test]
    fn inspect_passes_through_v1_envelope_verbatim() {
        let dir = test_dir("passthrough");
        let body = r#"{"schemaVersion":"codelattice.workspaceInspection.v1","root":"x","projects":[{"relativePath":"backend","language":"rust","confidence":"certain","evidence":{"kind":"manifest","file":"Cargo.toml"},"sourceFileCount":2,"analyzable":true}],"sourceOnlyAreas":[],"unsupportedAreas":[]}"#;
        let script = fake_cli(&dir, "ok", body);
        let envelope = inspect_with_cli(&script, "/whatever").unwrap();
        // 原样透传：不做字段增删
        assert_eq!(envelope, serde_json::from_str::<Value>(body).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_rejects_wrong_schema_version_naming_actual_value() {
        let dir = test_dir("schema");
        let script = fake_cli(&dir, "bad", r#"{"schemaVersion":"0.3.0","graph":{}}"#);
        let err = inspect_with_cli(&script, "/whatever").unwrap_err();
        assert!(
            err.contains("unexpected inspect schemaVersion"),
            "err={err}"
        );
        assert!(err.contains("0.3.0"), "必须点名实际值: {err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_reports_nonzero_exit_with_stderr_summary() {
        let dir = test_dir("exit");
        let script = dir.join("fail.sh");
        fs::write(
            &script,
            "#!/bin/sh\necho 'boom: root missing' >&2\nexit 3\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        let err = inspect_with_cli(&script, "/whatever").unwrap_err();
        assert!(err.contains("inspect failed"), "err={err}");
        assert!(err.contains("boom: root missing"), "err={err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
