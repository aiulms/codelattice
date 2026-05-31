//! 新增 productization CLI 命令的集成测试
//!
//! 测试 analyze / quality / summary 三个 productization 入口命令。
//! 使用 assert_cmd 库测试 CLI 行为：成功/失败路径、exit codes、JSON 结构。
//!
//! 测试策略：
//! - Rust 路径：使用 fixtures/rust/portable-smoke
//! - Cangjie 路径（feature-gated）：使用 fixtures/cangjie/portable-smoke
//! - 错误路径：不存在的 root / 不支持的语言

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

// 使用 Command::cargo_bin() 自动解析 binary 路径，无需手动 cli_binary()

/// fixtures 的 portable-smoke Rust 项目路径
fn rust_portable_smoke_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    format!("{manifest_dir}/../../fixtures/rust/portable-smoke")
}

/// fixtures 的 portable-smoke Cangjie 项目路径（仅在 feature-gated 测试中使用）
#[allow(dead_code)]
fn cangjie_portable_smoke_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    format!("{manifest_dir}/../../fixtures/cangjie/portable-smoke")
}

fn create_multi_project_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let root = dir.path();

    let rust_src = root.join("rust-app/src");
    std::fs::create_dir_all(&rust_src).unwrap();
    std::fs::write(
        root.join("rust-app/Cargo.toml"),
        "[package]\nname = \"rust-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(rust_src.join("main.rs"), "fn main() {}\n").unwrap();
    let rust_internal = root.join("rust-app/src/internal");
    std::fs::create_dir_all(&rust_internal).unwrap();
    std::fs::write(rust_internal.join("alpha.rs"), "pub fn alpha() {}\n").unwrap();
    std::fs::write(rust_internal.join("beta.rs"), "pub fn beta() {}\n").unwrap();

    let py = root.join("python-tool");
    std::fs::create_dir_all(&py).unwrap();
    std::fs::write(
        py.join("pyproject.toml"),
        "[project]\nname = \"python-tool\"\n",
    )
    .unwrap();
    std::fs::write(py.join("main.py"), "def main():\n    return 1\n").unwrap();

    let unsupported = root.join("csharp-addon");
    std::fs::create_dir_all(&unsupported).unwrap();
    std::fs::write(
        unsupported.join("csharp-addon.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n",
    )
    .unwrap();

    dir
}

fn create_dependency_rust_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create dependency project");
    let root = dir.path();
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "dependency-cli-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = "1"
serde = { version = "1", features = ["derive"] }
"#,
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "pub fn main_entry() { handler(); }\npub fn handler() {}\n",
    )
    .unwrap();
    dir
}

fn create_detect_changes_git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let root = dir.path();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("lib.rs"),
        r#"pub fn helper() -> i32 {
    41
}

pub fn entry() -> i32 {
    helper()
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "detect-changes-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    for args in [
        vec!["init"],
        vec!["config", "user.email", "test@test.com"],
        vec!["config", "user.name", "Test"],
        vec!["add", "."],
        vec!["commit", "-m", "baseline"],
    ] {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git command failed");
        assert!(
            output.status.success(),
            "git command failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    dir
}

#[test]
fn codelattice_binary_alias_runs_analyze() {
    let mut cmd = Command::cargo_bin("codelattice").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["language"], "rust");
    assert!(
        v["summary"]["symbolCount"].as_u64().unwrap() > 0,
        "codelattice alias 应执行同一 analyze 命令并产出符号统计"
    );
}

#[test]
fn codelattice_version_uses_public_name() {
    let mut cmd = Command::cargo_bin("codelattice").unwrap();

    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("codelattice"));
}

#[test]
fn detect_changes_reports_changed_symbols() {
    let dir = create_detect_changes_git_repo();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        r#"pub fn helper() -> i32 {
    99
}

pub fn entry() -> i32 {
    helper()
}
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/new_module.rs"),
        "pub fn new_helper() {}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codelattice").unwrap();
    let assert = cmd
        .arg("detect-changes")
        .arg("--root")
        .arg(dir.path())
        .arg("--language")
        .arg("rust")
        .arg("--scope")
        .arg("all")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["schemaVersion"], "codelattice.detectChanges.v1");
    assert_eq!(v["language"], "rust");
    assert_eq!(v["diffMode"], "head");
    assert!(
        v["summary"]["changedFileCount"].as_u64().unwrap_or(0) > 0,
        "应报告变更文件"
    );
    assert_eq!(
        v["summary"]["untrackedFileCount"].as_u64().unwrap_or(0),
        1,
        "scope=all 应报告未跟踪新文件"
    );
    assert!(v["untrackedFiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file.as_str() == Some("src/new_module.rs")));
    assert!(
        v["summary"]["changedSymbolCount"].as_u64().unwrap_or(0) > 0,
        "应报告变更符号: {v:?}"
    );
    assert!(
        v["changedSymbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|sym| sym["name"].as_str() == Some("helper")),
        "应识别 helper 变更: {v:?}"
    );
    assert_eq!(v["generatedFrom"]["nativeCodeLattice"], true);
    assert_eq!(v["generatedFrom"]["runtimeVerified"], false);
    assert!(v["underlyingTools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool.as_str() == Some("codelattice_changed_symbols")));
}

#[test]
fn detect_changes_non_git_repo_exits_nonzero() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("lib.rs"), "pub fn helper() {}\n").unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"not-git\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("codelattice").unwrap();
    cmd.arg("detect-changes")
        .arg("--root")
        .arg(dir.path())
        .arg("--language")
        .arg("rust")
        .assert()
        .failure()
        .stderr(predicate::str::contains("changed_symbols"));
}

// ============================================================
// analyze 命令 — Rust
// ============================================================

#[test]
fn analyze_rust_auto_detects_language() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["language"], "rust");
    assert!(
        v["summary"]["nodeCount"].as_u64().unwrap() > 0,
        "节点数应大于 0"
    );
    assert!(
        v["summary"]["edgeCount"].as_u64().unwrap() > 0,
        "边数应大于 0"
    );
    assert!(
        v["summary"]["symbolCount"].as_u64().unwrap() > 0,
        "符号数应大于 0"
    );
    assert!(v["graph"].is_object(), "graph 字段应为 JSON 对象");
    assert!(v["qualityGates"].is_array(), "qualityGates 应为数组");
    assert!(
        !v["qualityGates"].as_array().unwrap().is_empty(),
        "qualityGates 不应为空"
    );
}

#[test]
fn analyze_auto_multi_project_root_returns_workspace_auto_entry() {
    let dir = create_multi_project_workspace();
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(dir.path())
        .arg("--language")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["schemaVersion"], "codelattice.workspaceAutoEntry.v1");
    assert_eq!(v["status"], "workspace_analyzed");
    assert_eq!(v["rootKind"], "workspace");
    assert_eq!(v["summary"]["supportedProjectCount"].as_u64(), Some(2));
    assert_eq!(v["summary"]["sourceOnlyAreaCount"].as_u64(), Some(1));
    assert_eq!(v["supportedProjects"].as_array().unwrap().len(), 2);
    assert!(v["supportedProjects"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["manifestFile"].as_str().is_some_and(|m| !m.is_empty())));
    assert_eq!(v["sourceOnlyAreas"]["summary"]["count"].as_u64(), Some(1));
    assert_eq!(
        v["workspaceSummary"]["projectCount"].as_u64(),
        Some(3),
        "workspace graph projectCount should count manifest-backed boundaries only"
    );
    assert!(v["unsupportedModules"].as_array().unwrap().len() >= 1);
    assert_eq!(v["generatedFrom"]["staticAnalysis"], true);
    assert_eq!(v["generatedFrom"]["scriptsExecuted"], false);
    assert_eq!(v["generatedFrom"]["projectContentRead"], false);
}

#[test]
fn analyze_profile_symbols_is_bounded_and_pageable() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .arg("--profile")
        .arg("symbols")
        .arg("--profile-page-size")
        .arg("3")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["schemaVersion"], "codelattice.analyzeSymbols.v1");
    assert!(v.get("graph").is_none(), "symbols profile must omit graph");
    assert_eq!(v["paging"]["page"].as_u64(), Some(0));
    assert_eq!(v["paging"]["pageSize"].as_u64(), Some(3));
    assert!(
        v["symbols"].as_array().unwrap().len() <= 3,
        "symbols page must be bounded"
    );
    assert!(
        v["paging"]["totalItems"].as_u64().unwrap_or(0)
            >= v["symbols"].as_array().unwrap().len() as u64
    );
    assert!(v["detailHint"]
        .as_str()
        .unwrap_or("")
        .contains("--profile-page"));
}

#[test]
fn analyze_profile_symbols_public_only_filters_visibility() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .arg("--profile")
        .arg("symbols")
        .arg("--public-only")
        .arg("--profile-page-size")
        .arg("50")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");
    let symbols = v["symbols"].as_array().unwrap();

    assert!(
        !symbols.is_empty(),
        "portable fixture should expose public symbols"
    );
    assert!(
        symbols.iter().all(|s| {
            let visibility = s["visibility"].as_str().unwrap_or("");
            visibility == "pub" || visibility == "public"
        }),
        "public-only must omit private symbols: {symbols:?}"
    );
}

#[test]
fn analyze_profile_modules_is_bounded_and_pageable() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .arg("--profile")
        .arg("modules")
        .arg("--profile-page-size")
        .arg("1")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["schemaVersion"], "codelattice.analyzeModules.v1");
    assert_eq!(v["modules"].as_array().unwrap().len(), 1);
    assert_eq!(v["paging"]["pageSize"].as_u64(), Some(1));
    assert!(v["paging"]["totalItems"].as_u64().unwrap_or(0) >= 1);
    assert!(v["detailHint"]
        .as_str()
        .unwrap_or("")
        .contains("--profile-page"));
}

#[test]
fn analyze_profile_deps_returns_static_dependency_digest() {
    let fixture = create_dependency_rust_project();
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(fixture.path())
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .arg("--profile")
        .arg("deps")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(
        v["schemaVersion"].as_str(),
        Some("codelattice.analyzeDependencies.v1")
    );
    assert!(v.get("graph").is_none(), "deps profile must omit graph");
    assert_eq!(
        v["dependencySummary"]["schemaVersion"].as_str(),
        Some("codelattice.dependencyFrameworkDigest.v1")
    );
    assert!(
        v["dependencySummary"]["topDependencies"]
            .as_array()
            .map(|items| items.iter().any(|dep| dep["name"].as_str() == Some("axum")))
            .unwrap_or(false),
        "deps profile should expose manifest dependencies: {v:?}"
    );
    assert_eq!(
        v["generatedFrom"]["targetCodeExecuted"].as_bool(),
        Some(false)
    );
}

#[test]
fn analyze_rust_explicit_language() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["language"], "rust");
}

#[test]
fn analyze_nonexistent_root_exits_nonzero() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();

    cmd.arg("analyze")
        .arg("--root")
        .arg("/nonexistent/path/12345")
        .arg("--language")
        .arg("auto")
        .assert()
        .failure()
        .stderr(predicate::str::contains("root 路径不存在"));
}

#[test]
fn analyze_unsupported_format_rejected() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    cmd.arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("yaml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("支持的格式"));
}

// ============================================================
// analyze 命令 — Rust strict 模式
// ============================================================

#[test]
fn analyze_rust_strict_passes_on_clean_fixture() {
    // --strict 模式下，clean fixture 应 exit 0 并输出合法 JSON
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .arg("--strict")
        .assert()
        .success(); // 所有质量门 pass → exit 0

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    // strict 不影响 JSON 输出内容
    assert_eq!(v["language"], "rust");
    assert!(v["summary"]["nodeCount"].as_u64().unwrap() > 0);
    assert!(v["qualityGates"].is_array());
    // 所有 quality gates 应通过
    for gate in v["qualityGates"].as_array().unwrap() {
        assert!(
            gate["passed"].as_bool().unwrap(),
            "所有质量门应通过: {}",
            gate["gateName"]
        );
    }
}

#[test]
fn analyze_rust_strict_with_bridge_format() {
    // --strict 应与 --format gitnexus-rc 兼容
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .arg("--strict")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");
    assert!(v["repository"].is_object(), "bridge 格式应有 repository");
    assert_eq!(v["language"], "rust");
}

// ============================================================
// analyze 命令 — Bridge 格式（Rust）
// ============================================================

#[test]
fn analyze_rust_bridge_format() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    // Bridge 格式特有字段
    assert!(v["repository"].is_object(), "应有 repository 对象");
    assert!(
        v["packages"].is_array() && !v["packages"].as_array().unwrap().is_empty(),
        "packages 应为非空数组"
    );
    assert!(
        v["sourceFiles"].is_array() && !v["sourceFiles"].as_array().unwrap().is_empty(),
        "sourceFiles 应为非空数组"
    );
    assert!(
        v["symbols"].is_array() && !v["symbols"].as_array().unwrap().is_empty(),
        "symbols 应为非空数组"
    );
    assert!(v["edges"].is_object(), "edges 应为对象（按类型分组）");

    // edges 应按类型分组
    let edges = &v["edges"];
    assert!(
        edges["defines"].is_array() && !edges["defines"].as_array().unwrap().is_empty(),
        "应有 defines 边"
    );

    // stats 应匹配
    assert!(v["stats"]["symbolCount"].as_u64().unwrap() > 0);
    assert!(v["stats"]["nodeCount"].as_u64().unwrap() > 0);

    // 语言标记
    assert_eq!(v["language"], "rust");
}

#[test]
fn analyze_rust_bridge_format_vs_json_same_language() {
    // 验证 bridge 格式和 json 格式的语言检测一致
    let mut cmd_json = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let mut cmd_bridge = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let out_json = cmd_json
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    let v_json: Value =
        serde_json::from_str(&String::from_utf8(out_json.get_output().stdout.clone()).unwrap())
            .unwrap();

    let out_bridge = cmd_bridge
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();
    let v_bridge: Value =
        serde_json::from_str(&String::from_utf8(out_bridge.get_output().stdout.clone()).unwrap())
            .unwrap();

    assert_eq!(v_json["language"], v_bridge["language"]);
    // sourceFile 数量应一致
    assert_eq!(
        v_json["summary"]["sourceFileCount"].as_u64().unwrap(),
        v_bridge["stats"]["sourceFileCount"].as_u64().unwrap(),
        "两种格式的 sourceFileCount 应一致"
    );
    // bridge 格式将 diagnostic 节点映射为 symbol（ANNOTATES 端点完整性），
    // workspace 节点映射为 package（CONTAINS_WORKSPACE/CONTAINS_PACKAGE 端点完整性），
    // 因此 bridge 的 symbolCount >= json 的 symbolCount，packageCount >= json 的 packageCount
    assert!(
        v_bridge["stats"]["symbolCount"].as_u64().unwrap()
            >= v_json["summary"]["symbolCount"].as_u64().unwrap(),
        "bridge symbolCount 应 >= json symbolCount（bridge 包含 diagnostic 符号）"
    );
    assert!(
        v_bridge["stats"]["packageCount"].as_u64().unwrap()
            >= v_json["summary"]["packageCount"].as_u64().unwrap(),
        "bridge packageCount 应 >= json packageCount（bridge 包含 workspace 包条目）"
    );
}

// ============================================================
// quality 命令 — Rust
// ============================================================

#[test]
fn quality_rust_all_gates_pass() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("quality")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .assert()
        .success(); // exit code 0 = all pass

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(v["language"], "rust");
    assert_eq!(v["overall"], "pass");
    assert!(
        v["gates"].as_array().unwrap().len() >= 5,
        "至少应有 5 个质量门"
    );
}

#[test]
fn quality_requires_explicit_language() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    cmd.arg("quality")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("auto")
        .assert()
        .failure()
        .stderr(predicate::str::contains("需要显式指定"));
}

// ============================================================
// summary 命令 — Rust
// ============================================================

#[test]
fn summary_rust_no_full_graph() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = rust_portable_smoke_path();

    let assert = cmd
        .arg("summary")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(v["language"], "rust");
    // summary 不应包含完整 graph
    assert!(v.get("graph").is_none(), "summary 不应包含完整 graph");
    // 应有 graphSummary 统计
    assert!(v["graphSummary"]["nodeCount"].as_u64().unwrap() > 0);
    assert!(v["graphSummary"]["symbolCount"].as_u64().unwrap() > 0);
    // 应有 qualitySummary
    assert!(v["qualitySummary"]["total"].as_u64().unwrap() > 0);
}

// ============================================================
// analyze 命令 — Cangjie（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_auto_detects_language() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(v["language"], "cangjie");
    assert!(
        v["summary"]["nodeCount"].as_u64().unwrap() > 0,
        "Cangjie 节点数应大于 0"
    );
    assert!(v["graph"].is_object());
}

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_explicit_language() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["language"], "cangjie");
    // Cangjie 质量门应包含 synthetic_nodes
    let gate_names: Vec<&str> = v["qualityGates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["gateName"].as_str().unwrap())
        .collect();
    assert!(
        gate_names.contains(&"synthetic_nodes"),
        "Cangjie 质量门应包含 synthetic_nodes"
    );
}

// ============================================================
// analyze 命令 — Cangjie Bridge 格式（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_bridge_format() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert!(v["repository"].is_object(), "应有 repository 对象");
    assert!(
        v["packages"].is_array() && !v["packages"].as_array().unwrap().is_empty(),
        "packages 应为非空数组"
    );
    assert!(
        v["sourceFiles"].is_array() && !v["sourceFiles"].as_array().unwrap().is_empty(),
        "sourceFiles 应为非空数组"
    );
    assert!(
        v["symbols"].is_array() && !v["symbols"].as_array().unwrap().is_empty(),
        "symbols 应为非空数组"
    );

    // Cangjie bridge 格式应有 uses edges
    let edges = &v["edges"];
    assert!(
        edges["uses"].is_array() && !edges["uses"].as_array().unwrap().is_empty(),
        "Cangjie bridge 格式应有 uses 边"
    );
    assert!(
        edges["defines"].is_array() && !edges["defines"].as_array().unwrap().is_empty(),
        "Cangjie bridge 格式应有 defines 边"
    );

    // 语言标记
    assert_eq!(v["language"], "cangjie");
}

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_bridge_edges_use_normalized_endpoints() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("gitnexus-rc")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    // 所有边的端点字段应为 sourceId / targetId（归一化后）
    for edge_type in &["defines", "uses", "contains", "owns", "imports"] {
        if let Some(edges) = v["edges"][edge_type].as_array() {
            for edge in edges {
                assert!(
                    edge["sourceId"].is_string(),
                    "{edge_type} edge 应有 sourceId 字段"
                );
                assert!(
                    edge["targetId"].is_string(),
                    "{edge_type} edge 应有 targetId 字段"
                );
                // 不应出现旧字段名
                assert!(
                    edge.get("source").is_none(),
                    "{edge_type} edge 不应有 source 字段（应为 sourceId）"
                );
                assert!(
                    edge.get("target").is_none(),
                    "{edge_type} edge 不应有 target 字段（应为 targetId）"
                );
            }
        }
    }
}

// ============================================================
// analyze 命令 — Cangjie strict 模式（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_strict_passes_on_clean_fixture() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("json")
        .arg("--strict")
        .assert()
        .success(); // 所有质量门 pass（synthetic=0 等）→ exit 0

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");

    assert_eq!(v["language"], "cangjie");
    for gate in v["qualityGates"].as_array().unwrap() {
        assert!(
            gate["passed"].as_bool().unwrap(),
            "Cangjie 所有质量门应通过: {}",
            gate["gateName"]
        );
    }
}

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn analyze_cangjie_strict_with_bridge_format() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("analyze")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("gitnexus-rc")
        .arg("--strict")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("stdout 必须是合法 JSON");
    assert!(v["repository"].is_object(), "bridge 格式应有 repository");
    assert_eq!(v["language"], "cangjie");
}

// ============================================================
// quality 命令 — Cangjie（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn quality_cangjie_all_gates_pass() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("quality")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("json")
        .assert()
        .success(); // exit code 0 = all pass

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(v["language"], "cangjie");
    assert_eq!(v["overall"], "pass");
}

// ============================================================
// summary 命令 — Cangjie（feature-gated）
// ============================================================

#[cfg(feature = "tree-sitter-cangjie")]
#[test]
fn summary_cangjie_no_full_graph() {
    let mut cmd = Command::cargo_bin("gitnexus-rust-core-cli").unwrap();
    let root = cangjie_portable_smoke_path();

    let assert = cmd
        .arg("summary")
        .arg("--root")
        .arg(&root)
        .arg("--language")
        .arg("cangjie")
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    assert!(v.get("graph").is_none(), "summary 不应包含完整 graph");
    assert!(v["graphSummary"]["nodeCount"].as_u64().unwrap() > 0);
}
