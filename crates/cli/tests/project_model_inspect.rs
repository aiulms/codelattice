//! CLI integration tests
//!
//! 验证 project-model inspect 命令的输出契约：
//! - 输出可解析 JSON
//! - 顶层 14 个字段全部存在
//! - manifest scanner 各场景正确输出
//! - root 不存在时 exit 非 0
//! - stdout 只包含 JSON，不混入 human logs

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn fixture_dir(name: &str) -> PathBuf {
    let mut base = std::env::current_dir().unwrap();
    // 从 crates/cli/ 向上找到 workspace root
    while !base.join("fixtures").exists() && base.parent().is_some() {
        base = base.parent().unwrap().to_path_buf();
    }
    base.join("fixtures").join("manifest-scanner").join(name)
}

fn inspect_fixture(name: &str) -> (String, serde_json::Value) {
    let dir = fixture_dir(name);
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须是可解析 JSON");
    (stdout, parsed)
}

// === 基础契约测试 ===

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
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须只包含 JSON，不混入 human logs");
}

// === Manifest scanner 场景测试 ===

#[test]
fn root_package_finds_one_package_with_lib_target() {
    let (_, parsed) = inspect_fixture("root-package");

    // packages count = 1
    let packages = parsed["packages"].as_array().unwrap();
    assert_eq!(packages.len(), 1, "root-package fixture 应有 1 个 package");

    // package name = "app"
    assert_eq!(packages[0]["name"].as_str(), Some("app"));

    // discoveryReason = "root-manifest"
    assert_eq!(
        packages[0]["discoveryReason"].as_str(),
        Some("root-manifest")
    );

    // targets 包含 lib
    let targets = parsed["targets"].as_array().unwrap();
    assert!(
        targets.iter().any(|t| t["kind"].as_str() == Some("lib")),
        "应有 lib target"
    );

    // diagnostics 不含 scan-not-implemented
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        !diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("project-model-scan-not-implemented")),
        "manifest scan 成功时不应有 scan-not-implemented diagnostic"
    );

    // workspaces 为空
    let workspaces = parsed["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 0);
}

#[test]
fn subdir_package_finds_backend() {
    let (_, parsed) = inspect_fixture("subdir-package");

    // packages 包含 backend
    let packages = parsed["packages"].as_array().unwrap();
    assert!(
        packages
            .iter()
            .any(|p| p["name"].as_str() == Some("backend")),
        "应有 backend package"
    );

    // discoveryReason = "subdirectory-scan"
    let backend = packages
        .iter()
        .find(|p| p["name"].as_str() == Some("backend"))
        .unwrap();
    assert_eq!(
        backend["discoveryReason"].as_str(),
        Some("subdirectory-scan")
    );
}

#[test]
fn virtual_workspace_explicit_finds_workspace_and_member() {
    let (_, parsed) = inspect_fixture("virtual-workspace-explicit");

    // workspaces count = 1
    let workspaces = parsed["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);

    // rawMembers 包含 "backend"
    let raw_members = workspaces[0]["rawMembers"].as_array().unwrap();
    assert!(
        raw_members.iter().any(|m| m.as_str() == Some("backend")),
        "rawMembers 应含 backend"
    );

    // expandedMembers 包含 "backend"
    let expanded_members = workspaces[0]["expandedMembers"].as_array().unwrap();
    assert!(
        expanded_members
            .iter()
            .any(|m| m.as_str() == Some("backend")),
        "expandedMembers 应含 backend"
    );

    // packages 包含 backend
    let packages = parsed["packages"].as_array().unwrap();
    assert!(
        packages
            .iter()
            .any(|p| p["name"].as_str() == Some("backend")),
        "应有 backend package"
    );

    // backend isWorkspaceMember = true
    let backend = packages
        .iter()
        .find(|p| p["name"].as_str() == Some("backend"))
        .unwrap();
    assert_eq!(backend["isWorkspaceMember"].as_bool(), Some(true));

    // discoveryReason = "workspace-explicit"
    assert_eq!(
        backend["discoveryReason"].as_str(),
        Some("workspace-explicit")
    );
}

#[test]
fn virtual_workspace_glob_expands_members() {
    let (_, parsed) = inspect_fixture("virtual-workspace-glob");

    // workspaces count = 1
    let workspaces = parsed["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);

    // expandedMembers 应包含 alpha 和 beta
    let expanded = workspaces[0]["expandedMembers"].as_array().unwrap();
    assert!(
        expanded.iter().any(|m| m.as_str() == Some("crates/alpha")),
        "expandedMembers 应含 crates/alpha"
    );
    assert!(
        expanded.iter().any(|m| m.as_str() == Some("crates/beta")),
        "expandedMembers 应含 crates/beta"
    );

    // packages 包含 alpha 和 beta
    let packages = parsed["packages"].as_array().unwrap();
    assert!(
        packages.iter().any(|p| p["name"].as_str() == Some("alpha")),
        "应有 alpha package"
    );
    assert!(
        packages.iter().any(|p| p["name"].as_str() == Some("beta")),
        "应有 beta package"
    );

    // discoveryReason = "workspace-glob"
    let alpha = packages
        .iter()
        .find(|p| p["name"].as_str() == Some("alpha"))
        .unwrap();
    assert_eq!(alpha["discoveryReason"].as_str(), Some("workspace-glob"));
}

#[test]
fn missing_member_produces_diagnostic() {
    let (_, parsed) = inspect_fixture("missing-member");

    // diagnostics 包含 workspace-member-path-missing
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("workspace-member-path-missing")),
        "应有 workspace-member-path-missing diagnostic"
    );
}

#[test]
fn complex_glob_produces_diagnostic() {
    let (_, parsed) = inspect_fixture("complex-glob");

    // diagnostics 包含 complex-glob-unsupported
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("complex-glob-unsupported")),
        "应有 complex-glob-unsupported diagnostic"
    );

    // partial = true
    assert_eq!(parsed["partial"].as_bool(), Some(true));
}

#[test]
fn missing_root_cargo_toml_produces_error_diagnostic() {
    // 使用 subdir-package 的 src 目录（没有 Cargo.toml）
    let dir = fixture_dir("root-package").join("src");
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(dir.to_string_lossy().as_ref())
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("cargo-toml-missing")),
        "无 Cargo.toml 时应有 cargo-toml-missing diagnostic"
    );

    // packages 为空
    let packages = parsed["packages"].as_array().unwrap();
    assert_eq!(packages.len(), 0);
}
