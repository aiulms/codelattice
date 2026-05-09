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
