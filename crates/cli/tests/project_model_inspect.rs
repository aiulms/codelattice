//! CLI integration tests
//!
//! 验证 project-model inspect 命令的输出契约：
//! - 输出可解析 JSON
//! - 顶层 14 个字段全部存在
//! - diagnostics 包含 scan-not-implemented
//! - root 不存在时 exit 非 0
//! - stdout 只包含 JSON，不混入 human logs

use assert_cmd::Command;
use predicates::prelude::*;

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

#[test]
fn inspect_current_dir_outputs_valid_json() {
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(".")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须是可解析 JSON");
    assert!(parsed.is_object(), "顶层必须是 JSON 对象");
}

#[test]
fn inspect_json_has_14_top_level_fields() {
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(".")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let obj = parsed.as_object().unwrap();

    let required_fields = [
        "version",
        "command",
        "repoRoot",
        "generatedAt",
        "projectModel",
        "packages",
        "workspaces",
        "targets",
        "sourceOwnership",
        "rootResolution",
        "diagnostics",
        "partial",
        "warnings",
        "stats",
    ];

    for field in &required_fields {
        assert!(obj.contains_key(*field), "缺少顶层字段: {field}");
    }
}

#[test]
fn inspect_diagnostics_contains_scan_not_implemented() {
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(".")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    let has_scan_not_implemented = diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("project-model-scan-not-implemented"));
    assert!(
        has_scan_not_implemented,
        "diagnostics 必须包含 project-model-scan-not-implemented"
    );
}

#[test]
fn inspect_nonexistent_root_exits_nonzero() {
    cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg("/nonexistent/path/that/does/not/exist")
        .arg("--format")
        .arg("json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("不存在"));
}

#[test]
fn inspect_stdout_is_pure_json_no_human_logs() {
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(".")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // 如果 stdout 是纯 JSON，解析不应失败
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须只包含 JSON，不混入 human logs");
    // stderr 可以为空或包含 human logs，不影响 stdout
}
