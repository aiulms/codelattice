//! CLI integration tests for ArkTS and TypeScript commands
//!
//! Verifies:
//! - ArkTS analyze: valid JSON, non-zero stats, quality gates pass
//! - ArkTS bridge: sourceId/targetId endpoints, no empty strings
//! - TypeScript analyze: valid JSON, non-zero stats
//! - stdout contains only JSON, no human logs
//! - feature gate: graceful failure when tree-sitter-arkts is disabled

use assert_cmd::Command;
use serde_json::Value;

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn find_workspace_root() -> std::path::PathBuf {
    let mut base = std::env::current_dir().unwrap();
    while !base.join("fixtures").exists() && base.parent().is_some() {
        base = base.parent().unwrap().to_path_buf();
    }
    base
}

fn arkts_fixture() -> std::path::PathBuf {
    find_workspace_root()
        .join("fixtures")
        .join("arkts")
        .join("portable-smoke")
}

fn ts_fixture() -> std::path::PathBuf {
    find_workspace_root()
        .join("fixtures")
        .join("typescript")
        .join("portable-smoke")
}

// ============================================================
// ArkTS analyze — JSON format
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn arkts_analyze_json_valid_output() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "analyze",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout is not valid JSON");

    assert_eq!(json["language"], "arkts");
    assert!(
        json["summary"]["sourceFileCount"].as_u64().unwrap() >= 2,
        "expected >= 2 source files, got {}",
        json["summary"]["sourceFileCount"]
    );
    assert!(
        json["summary"]["symbolCount"].as_u64().unwrap() > 0,
        "expected symbolCount > 0"
    );
    assert!(
        json["summary"]["edgeCount"].as_u64().unwrap() > 0,
        "expected edgeCount > 0"
    );
}

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn arkts_analyze_quality_gates_pass() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "analyze",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout is not valid JSON");

    let gates = json["qualityGates"]
        .as_array()
        .expect("qualityGates is array");
    assert!(!gates.is_empty(), "quality gates should not be empty");
    for gate in gates {
        assert!(
            gate["passed"].as_bool().unwrap_or(false),
            "quality gate '{}' failed: {}",
            gate["gateName"],
            gate["detail"]
        );
    }
}

// ============================================================
// ArkTS bridge format
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn arkts_bridge_format_valid() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "analyze",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
            "--format",
            "gitnexus-rc",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("bridge output is not valid JSON");

    assert_eq!(json["language"], "arkts");
    assert!(
        json["stats"]["nodeCount"].as_u64().unwrap() > 0,
        "bridge stats nodeCount should be > 0"
    );
    assert!(
        json["stats"]["sourceFileCount"].as_u64().unwrap() >= 2,
        "bridge stats sourceFileCount should be >= 2"
    );

    // Verify sourceId/targetId are non-empty
    let edges = &json["edges"];
    for (kind, arr) in edges.as_object().unwrap_or(&serde_json::Map::new()) {
        if kind == "other" {
            continue;
        }
        if let Some(edge_list) = arr.as_array() {
            for edge in edge_list {
                let source_id = edge["sourceId"].as_str().unwrap_or("");
                let target_id = edge["targetId"].as_str().unwrap_or("");
                assert!(
                    !source_id.is_empty(),
                    "edge kind '{}' has empty sourceId: {:?}",
                    kind,
                    edge
                );
                assert!(
                    !target_id.is_empty(),
                    "edge kind '{}' has empty targetId: {:?}",
                    kind,
                    edge
                );
            }
        }
    }
}

// ============================================================
// ArkTS quality command
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn arkts_quality_command() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "quality",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("quality output is not valid JSON");

    assert_eq!(json["overall"], "pass");
    let gates = json["gates"].as_array().expect("gates is array");
    assert!(!gates.is_empty());
}

// ============================================================
// ArkTS summary command
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn arkts_summary_command() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "summary",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("summary output is not valid JSON");

    assert_eq!(json["language"], "arkts");
    assert!(
        json["graphSummary"]["nodeCount"].as_u64().unwrap() > 0,
        "summary nodeCount should be > 0"
    );
    assert_eq!(
        json["qualitySummary"]["total"].as_u64().unwrap(),
        json["qualitySummary"]["passed"].as_u64().unwrap(),
        "all quality gates should pass"
    );
}

// ============================================================
// TypeScript analyze — JSON format
// ============================================================

#[cfg(feature = "tree-sitter-arkts")]
#[test]
fn typescript_analyze_json_valid_output() {
    let root = ts_fixture();
    let output = cli_bin()
        .args([
            "analyze",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "typescript",
        ])
        .output()
        .expect("failed to run CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).expect("stdout is not valid JSON");

    assert_eq!(json["language"], "typescript");
    assert!(
        json["summary"]["sourceFileCount"].as_u64().unwrap() >= 2,
        "expected >= 2 source files, got {}",
        json["summary"]["sourceFileCount"]
    );
    assert!(
        json["summary"]["symbolCount"].as_u64().unwrap() > 0,
        "expected symbolCount > 0"
    );
}

// ============================================================
// Feature gate: graceful failure when disabled
// ============================================================

#[cfg(not(feature = "tree-sitter-arkts"))]
#[test]
fn arkts_disabled_graceful_error() {
    let root = arkts_fixture();
    let output = cli_bin()
        .args([
            "analyze",
            "--root",
            root.to_str().unwrap(),
            "--language",
            "arkts",
        ])
        .output()
        .expect("failed to run CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("disabled") || stderr.contains("features"),
        "stderr should mention disabled/features: {}",
        stderr
    );
}
