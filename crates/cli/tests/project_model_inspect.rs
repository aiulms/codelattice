//! CLI integration tests
//!
//! 验证 project-model inspect 命令的输出契约：
//! - 输出可解析 JSON
//! - 顶层 14 个字段全部存在
//! - manifest scanner 各场景正确输出
//! - source ownership 各场景正确输出
//! - root 不存在时 exit 非 0
//! - stdout 只包含 JSON，不混入 human logs

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn manifest_fixture_dir(name: &str) -> PathBuf {
    let base = find_workspace_root();
    base.join("fixtures").join("manifest-scanner").join(name)
}

fn source_fixture_dir(name: &str) -> PathBuf {
    let base = find_workspace_root();
    base.join("fixtures").join("source-ownership").join(name)
}

fn root_fixture_dir(name: &str) -> PathBuf {
    let base = find_workspace_root();
    base.join("fixtures").join("root-resolution").join(name)
}

fn find_workspace_root() -> PathBuf {
    let mut base = std::env::current_dir().unwrap();
    while !base.join("fixtures").exists() && base.parent().is_some() {
        base = base.parent().unwrap().to_path_buf();
    }
    base
}

fn inspect_dir(dir: &PathBuf) -> (String, serde_json::Value) {
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

fn inspect_manifest(name: &str) -> (String, serde_json::Value) {
    inspect_dir(&manifest_fixture_dir(name))
}

fn inspect_source(name: &str) -> (String, serde_json::Value) {
    inspect_dir(&source_fixture_dir(name))
}

fn inspect_root(name: &str) -> (String, serde_json::Value) {
    inspect_dir(&root_fixture_dir(name))
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
fn inspect_json_has_all_top_level_fields() {
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
        // item/symbol 扩展字段
        "symbols",
        "symbolDiagnostics",
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
    let (_, parsed) = inspect_manifest("root-package");
    let packages = parsed["packages"].as_array().unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0]["name"].as_str(), Some("app"));
}

#[test]
fn subdir_package_finds_backend() {
    let (_, parsed) = inspect_manifest("subdir-package");
    let packages = parsed["packages"].as_array().unwrap();
    assert!(packages
        .iter()
        .any(|p| p["name"].as_str() == Some("backend")));
}

#[test]
fn virtual_workspace_explicit_finds_workspace_and_member() {
    let (_, parsed) = inspect_manifest("virtual-workspace-explicit");
    let workspaces = parsed["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);
    let expanded = workspaces[0]["expandedMembers"].as_array().unwrap();
    assert!(expanded.iter().any(|m| m.as_str() == Some("backend")));
}

#[test]
fn virtual_workspace_glob_expands_members() {
    let (_, parsed) = inspect_manifest("virtual-workspace-glob");
    let expanded = parsed["workspaces"][0]["expandedMembers"]
        .as_array()
        .unwrap();
    assert!(expanded.iter().any(|m| m.as_str() == Some("crates/alpha")));
    assert!(expanded.iter().any(|m| m.as_str() == Some("crates/beta")));
}

#[test]
fn missing_member_produces_diagnostic() {
    let (_, parsed) = inspect_manifest("missing-member");
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("workspace-member-path-missing")));
}

#[test]
fn complex_glob_produces_diagnostic() {
    let (_, parsed) = inspect_manifest("complex-glob");
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("complex-glob-unsupported")));
    assert_eq!(parsed["partial"].as_bool(), Some(true));
}

#[test]
fn missing_root_cargo_toml_produces_error_diagnostic() {
    let dir = manifest_fixture_dir("root-package").join("src");
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
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("cargo-toml-missing")));
}

// === Source ownership 场景测试 ===

#[test]
fn so_root_package_lib_and_bin_owned() {
    let (_, parsed) = inspect_source("root-package");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    // 应有 3 个 .rs 文件
    assert!(so.len() >= 3, "至少应有 3 个 sourceOwnership 条目");

    // src/lib.rs → package=app, target=app(lib)
    let lib_rs = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/lib.rs"))
        .unwrap();
    assert_eq!(lib_rs["package"].as_str(), Some("app"));
    assert_eq!(
        lib_rs["ownershipReason"].as_str(),
        Some("source-owned-by-lib-target-root")
    );
    assert_eq!(lib_rs["confidence"].as_f64(), Some(0.90));

    // src/main.rs → package=app, bin target
    let main_rs = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/main.rs"))
        .unwrap();
    assert_eq!(main_rs["package"].as_str(), Some("app"));
    assert_eq!(
        main_rs["ownershipReason"].as_str(),
        Some("source-owned-by-bin-target-root")
    );
}

#[test]
fn so_subdir_package_backend_owned() {
    let (_, parsed) = inspect_source("subdir-package");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    let lib = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("backend/src/lib.rs"))
        .unwrap();
    assert_eq!(lib["package"].as_str(), Some("backend"));
}

#[test]
fn so_named_bin_target() {
    let (_, parsed) = inspect_source("named-bin");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    let worker = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/bin/worker.rs"))
        .unwrap();
    assert_eq!(worker["package"].as_str(), Some("worker"));
    assert_eq!(worker["target"].as_str(), Some("worker"));
    assert_eq!(
        worker["ownershipReason"].as_str(),
        Some("source-owned-by-named-bin-target-root")
    );
}

#[test]
fn so_single_target_shared归属lib() {
    let (_, parsed) = inspect_source("single-target-shared");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    let common = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/common.rs"))
        .unwrap();
    assert_eq!(common["package"].as_str(), Some("app"));
    // 单 target package，common.rs 归入 lib target
    // confidence 0.90：单 target 归属是确定性推理（同 ExactTarget 同档）
    assert_eq!(common["confidence"].as_f64(), Some(0.90));
}

#[test]
fn so_lib_and_bin_shared_ambiguous() {
    let (_, parsed) = inspect_source("lib-and-bin-shared");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    let common = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/common.rs"))
        .unwrap();
    assert_eq!(common["package"].as_str(), Some("app"));
    // 多 target package，common.rs target 不确定
    assert_eq!(common["target"].as_str(), None);
    assert_eq!(
        common["ownershipReason"].as_str(),
        Some("source-target-ambiguous")
    );
    assert_eq!(common["confidence"].as_f64(), Some(0.50));

    // 应有 source-target-ambiguous diagnostic
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("source-target-ambiguous")),
        "应有 source-target-ambiguous diagnostic"
    );
}

#[test]
fn so_nested_package_nearest_wins() {
    let (_, parsed) = inspect_source("nested-package");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    // backend/tools/src/lib.rs 应归属 tools，不是 backend
    let tool_lib = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("backend/tools/src/lib.rs"))
        .unwrap();
    assert_eq!(tool_lib["package"].as_str(), Some("tools"));
}

#[test]
fn so_outside_package_no_owner() {
    let (_, parsed) = inspect_source("outside-package");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    let setup = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("scripts/setup.rs"))
        .unwrap();
    assert_eq!(setup["package"].as_str(), None);
    assert_eq!(setup["target"].as_str(), None);
    assert_eq!(
        setup["ownershipReason"].as_str(),
        Some("source-outside-package")
    );

    // 应有 source-outside-package diagnostic
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"].as_str() == Some("source-outside-package")),
        "应有 source-outside-package diagnostic"
    );
}

#[test]
fn so_virtual_workspace_stray_outside() {
    let (_, parsed) = inspect_source("virtual-workspace-stray");

    let so = parsed["sourceOwnership"].as_array().unwrap();
    // root/src/lib.rs 在 virtual workspace root 下，不属于任何 package
    let stray = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("src/lib.rs"))
        .unwrap();
    assert_eq!(stray["package"].as_str(), None);

    // crates/member/src/lib.rs 应归属 member
    let member_lib = so
        .iter()
        .find(|s| s["sourcePath"].as_str() == Some("crates/member/src/lib.rs"))
        .unwrap();
    assert_eq!(member_lib["package"].as_str(), Some("member"));
}

#[test]
fn so_root_resolution_empty_without_queries() {
    let (_, parsed) = inspect_source("root-package");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 0, "无 root-queries.txt 时 rootResolution 应为空");
}

#[test]
fn so_stats_reflect_counts() {
    let (_, parsed) = inspect_source("root-package");
    let stats = &parsed["stats"];
    let source_count = stats["sourceFileCount"].as_u64().unwrap();
    let owned_count = stats["ownedFileCount"].as_u64().unwrap();
    assert!(source_count > 0, "sourceFileCount 应 > 0");
    assert!(owned_count > 0, "ownedFileCount 应 > 0");
    assert_eq!(source_count, owned_count, "所有 .rs 都应有 package owner");
}

// === Item/Symbol Model 测试 ===

#[test]
fn inspect_has_symbols_and_symbol_diagnostics_fields() {
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

    // 即使不加 --include symbols，也应有两个 additive 字段
    assert!(obj.contains_key("symbols"), "应有 symbols 字段");
    assert!(
        obj.contains_key("symbolDiagnostics"),
        "应有 symbolDiagnostics 字段"
    );
}

#[test]
fn inspect_without_include_symbols_has_empty_symbols() {
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

    let symbols = parsed["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 0, "不加 --include symbols 时 symbols 应为空");

    let symbol_diagnostics = parsed["symbolDiagnostics"].as_array().unwrap();
    assert_eq!(symbol_diagnostics.len(), 0, "symbolDiagnostics 应为空");
}

#[test]
fn inspect_with_include_symbols_has_empty_but_present_symbols() {
    let output = cli_bin()
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(".")
        .arg("--format")
        .arg("json")
        .arg("--include")
        .arg("symbols")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let symbols = parsed["symbols"].as_array().unwrap();
    // 第一刀不做真实 extraction，symbols 为空
    assert_eq!(
        symbols.len(),
        0,
        "第一刀 NoopItemExtractor 不做真实 extraction"
    );

    let symbol_diagnostics = parsed["symbolDiagnostics"].as_array().unwrap();
    assert_eq!(symbol_diagnostics.len(), 0);

    // stats.symbolCount 应为 0
    let symbol_count = parsed["stats"]["symbolCount"].as_u64().unwrap();
    assert_eq!(symbol_count, 0);
}

#[test]
fn inspect_stats_has_symbol_count_field() {
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

    let stats = &parsed["stats"];
    assert!(stats.as_object().unwrap().contains_key("symbolCount"));
    assert_eq!(stats["symbolCount"].as_u64(), Some(0));
}

// === Root resolution 场景测试 ===

#[test]
fn rr_lib_crate_root() {
    let (_, parsed) = inspect_root("lib-crate-root");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    let entry = &rr[0];
    assert_eq!(entry["sourcePath"].as_str(), Some("src/lib.rs"));
    assert_eq!(entry["queryPath"].as_str(), Some("crate::models"));
    assert_eq!(entry["resolvedPath"].as_str(), Some("src/models.rs"));
    assert_eq!(entry["targetKind"].as_str(), Some("lib"));
    assert_eq!(
        entry["rootReason"].as_str(),
        Some("module-declaration-resolved")
    );
    assert_eq!(entry["confidence"].as_f64(), Some(0.85));
    assert_eq!(entry["resolvedKind"].as_str(), Some("module"));
    assert_eq!(entry["crateRootFile"].as_str(), Some("./src/lib.rs"));
}

#[test]
fn rr_bin_crate_root() {
    let (_, parsed) = inspect_root("bin-crate-root");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), Some("src/app.rs"));
    assert_eq!(rr[0]["targetKind"].as_str(), Some("bin"));
}

#[test]
fn rr_named_bin_crate_root() {
    let (_, parsed) = inspect_root("named-bin-crate-root");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(
        rr[0]["resolvedPath"].as_str(),
        Some("src/bin/worker_local.rs")
    );
    assert_eq!(rr[0]["targetKind"].as_str(), Some("bin"));
}

#[test]
fn rr_chained_module() {
    let (_, parsed) = inspect_root("chained-module");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), Some("src/api/models.rs"));
    assert_eq!(rr[0]["rootReason"].as_str(), Some("module-chain-resolved"));
    assert_eq!(rr[0]["confidence"].as_f64(), Some(0.80));
}

#[test]
fn rr_missing_mod_declaration() {
    let (_, parsed) = inspect_root("missing-mod-declaration");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), None);
    assert_eq!(rr[0]["rootReason"].as_str(), Some("module-not-declared"));
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("module-not-declared")));
}

#[test]
fn rr_missing_module_file() {
    let (_, parsed) = inspect_root("missing-module-file");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), None);
    assert_eq!(rr[0]["rootReason"].as_str(), Some("module-file-missing"));
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("module-file-missing")));
}

#[test]
fn rr_ambiguous_module_file() {
    let (_, parsed) = inspect_root("ambiguous-module-file");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), None);
    assert_eq!(rr[0]["rootReason"].as_str(), Some("crate-path-ambiguous"));
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("crate-path-ambiguous")));
}

#[test]
fn rr_source_target_ambiguous_skipped() {
    let (_, parsed) = inspect_root("source-target-ambiguous");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(rr[0]["resolvedPath"].as_str(), None);
    assert_eq!(
        rr[0]["rootReason"].as_str(),
        Some("root-resolution-skipped")
    );
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("root-resolution-skipped")));
}

#[test]
fn rr_nested_package_root() {
    let (_, parsed) = inspect_root("nested-package-root");
    let rr = parsed["rootResolution"].as_array().unwrap();
    assert_eq!(rr.len(), 1);
    assert_eq!(
        rr[0]["resolvedPath"].as_str(),
        Some("backend/tools/src/helpers.rs")
    );
}

#[test]
fn rr_stats_reflect_resolution_counts() {
    let (_, parsed) = inspect_root("lib-crate-root");
    let stats = &parsed["stats"];
    let success = stats["resolutionSuccessCount"].as_u64().unwrap();
    let fail = stats["resolutionFailCount"].as_u64().unwrap();
    assert_eq!(success, 1);
    assert_eq!(fail, 0);
}
