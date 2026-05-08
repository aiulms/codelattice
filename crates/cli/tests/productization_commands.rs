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
        .stderr(predicate::str::contains("仅支持 --format json"));
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
