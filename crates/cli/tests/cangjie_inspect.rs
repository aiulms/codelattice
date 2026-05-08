//! CLI integration tests for Cangjie commands
//!
//! 验证 Cangjie inspect/graph 命令的输出契约：
//! - 输出可解析 JSON
//! - 节点/边类型覆盖完整
//! - 非 feature 时 graceful failure
//! - stdout 只包含 JSON，不混入 human logs

use assert_cmd::Command;
#[cfg(feature = "tree-sitter-cangjie")]
use predicates::prelude::*;

fn cli_bin() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

fn cangjie_fixture_dir(name: &str) -> std::path::PathBuf {
    let base = find_workspace_root();
    base.join("fixtures").join("cangjie").join(name)
}

fn find_workspace_root() -> std::path::PathBuf {
    let mut base = std::env::current_dir().unwrap();
    while !base.join("fixtures").exists() && base.parent().is_some() {
        base = base.parent().unwrap().to_path_buf();
    }
    base
}

// === 基础契约测试 ===

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_help_lists_subcommands() {
    cli_bin()
        .arg("cangjie")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("inspect"))
        .stdout(predicate::str::contains("graph"));
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_outputs_valid_json() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须是可解析 JSON");
    assert!(parsed.is_object(), "顶层必须是 JSON 对象");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_outputs_valid_json() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须是可解析 JSON");
    assert!(parsed.is_object(), "顶层必须是 JSON 对象");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_has_required_fields() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let obj = parsed.as_object().unwrap();

    // CangjieGraphOutput 应有 nodes 和 edges 字段
    assert!(obj.contains_key("nodes"), "缺少 nodes 字段");
    assert!(obj.contains_key("edges"), "缺少 edges 字段");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_nonexistent_root_exits_nonzero() {
    cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("不存在"));
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_stdout_is_pure_json_no_human_logs() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout 必须只包含 JSON，不混入 human logs");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_graph_has_nodes_and_edges() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let nodes = parsed["nodes"].as_array().expect("nodes 应为数组");
    let edges = parsed["edges"].as_array().expect("edges 应为数组");

    // 应有至少一些节点和边
    assert!(!nodes.is_empty(), "nodes 应非空");
    assert!(!edges.is_empty(), "edges 应非空");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_has_expected_node_types() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let nodes = parsed["nodes"].as_array().expect("nodes 应为数组");
    let node_kinds: Vec<&str> = nodes.iter().filter_map(|n| n["kind"].as_str()).collect();

    // 应包含预期的节点类型
    assert!(node_kinds.contains(&"repository"), "应有 repository 节点");
    assert!(node_kinds.contains(&"package"), "应有 package 节点");
    assert!(node_kinds.contains(&"sourceFile"), "应有 sourceFile 节点");
    assert!(node_kinds.contains(&"symbol"), "应有 symbol 节点");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_has_expected_edge_types() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let edges = parsed["edges"].as_array().expect("edges 应为数组");
    let edge_kinds: Vec<&str> = edges.iter().filter_map(|e| e["kind"].as_str()).collect();

    // 应包含预期的边类型
    assert!(
        edge_kinds.contains(&"containsPackage"),
        "应有 containsPackage 边"
    );
    assert!(edge_kinds.contains(&"ownsSource"), "应有 ownsSource 边");
    assert!(edge_kinds.contains(&"defines"), "应有 defines 边");
}

// === Graph 命令测试 ===

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_same_output_as_inspect() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");

    let inspect_output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let graph_output = cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    let inspect_stdout = String::from_utf8_lossy(&inspect_output.stdout);
    let graph_stdout = String::from_utf8_lossy(&graph_output.stdout);

    // inspect 和 graph 应输出相同的内容
    assert_eq!(
        inspect_stdout, graph_stdout,
        "inspect 和 graph 应输出相同内容"
    );
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_nonexistent_root_exits_nonzero() {
    cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("不存在"));
}

// === Feature gate 测试 ===

#[test]
#[cfg(not(feature = "tree-sitter-cangjie"))]
fn cangjie_inspect_disabled_feature_error() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    // 应该非 0 退出
    assert!(!output.status.success(), "应该非 0 退出");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // 应该包含 feature disabled 的错误信息
    assert!(
        stderr.contains("Cangjie support is disabled"),
        "stderr 应该包含 'Cangjie support is disabled'，实际: {}",
        stderr
    );
    assert!(
        stderr.contains("--features tree-sitter-cangjie"),
        "stderr 应该包含 '--features tree-sitter-cangjie'，实际: {}",
        stderr
    );
}

#[test]
#[cfg(not(feature = "tree-sitter-cangjie"))]
fn cangjie_graph_disabled_feature_error() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .output()
        .unwrap();

    // 应该非 0 退出
    assert!(!output.status.success(), "应该非 0 退出");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // 应该包含 feature disabled 的错误信息
    assert!(
        stderr.contains("Cangjie support is disabled"),
        "stderr 应该包含 'Cangjie support is disabled'，实际: {}",
        stderr
    );
    assert!(
        stderr.contains("--features tree-sitter-cangjie"),
        "stderr 应该包含 '--features tree-sitter-cangjie'，实际: {}",
        stderr
    );
}

// === --strict quality gate 测试 ===

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_strict_on_portable_smoke_succeeds() {
    let fixture_dir = cangjie_fixture_dir("portable-smoke");
    cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .assert()
        .success();
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_strict_stdout_is_pure_json() {
    let fixture_dir = cangjie_fixture_dir("portable-smoke");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value =
        serde_json::from_str(&stdout).expect("--strict stdout 必须只包含 JSON");
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_strict_equals_inspect_strict() {
    let fixture_dir = cangjie_fixture_dir("portable-smoke");

    let inspect_output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .output()
        .unwrap();

    let graph_output = cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .output()
        .unwrap();

    let inspect_stdout = String::from_utf8_lossy(&inspect_output.stdout);
    let graph_stdout = String::from_utf8_lossy(&graph_output.stdout);

    assert_eq!(
        inspect_stdout, graph_stdout,
        "inspect --strict 和 graph --strict 应输出相同内容"
    );
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_strict_nonexistent_root_exits_nonzero() {
    cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg("/nonexistent/path/that/does/not/exist")
        .arg("--strict")
        .assert()
        .failure()
        .stderr(predicate::str::contains("不存在"));
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_strict_nonexistent_root_exits_nonzero() {
    cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg("/nonexistent/path/that/does/not/exist")
        .arg("--strict")
        .assert()
        .failure()
        .stderr(predicate::str::contains("不存在"));
}

#[test]
#[cfg(not(feature = "tree-sitter-cangjie"))]
fn cangjie_inspect_strict_disabled_feature_error() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "feature disabled + --strict 应该非 0 退出"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cangjie support is disabled"),
        "stderr 应该包含 'Cangjie support is disabled'"
    );
    assert!(
        stderr.contains("--features tree-sitter-cangjie"),
        "stderr 应该包含 '--features tree-sitter-cangjie'"
    );
}

#[test]
#[cfg(not(feature = "tree-sitter-cangjie"))]
fn cangjie_graph_strict_disabled_feature_error() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    let output = cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .arg("--strict")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "feature disabled + --strict 应该非 0 退出"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cangjie support is disabled"),
        "stderr 应该包含 'Cangjie support is disabled'"
    );
    assert!(
        stderr.contains("--features tree-sitter-cangjie"),
        "stderr 应该包含 '--features tree-sitter-cangjie'"
    );
}

// === 实际功能测试 ===

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_inspect_on_basic_fixture_succeeds() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    cli_bin()
        .arg("cangjie")
        .arg("inspect")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .assert()
        .success();
}

#[test]
#[cfg(feature = "tree-sitter-cangjie")]
fn cangjie_graph_on_basic_fixture_succeeds() {
    let fixture_dir = cangjie_fixture_dir("imports-basic");
    cli_bin()
        .arg("cangjie")
        .arg("graph")
        .arg("--root")
        .arg(fixture_dir.to_string_lossy().as_ref())
        .assert()
        .success();
}
