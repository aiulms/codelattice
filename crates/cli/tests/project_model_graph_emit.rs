//! Graph emitter integration tests
//!
//! 验证 --include graph 输出的 GraphOutput 符合 graph-schema-v0 定义。
//! 3 个 MVP fixtures：
//!   A) root-package — 单 package + lib target + SourceFile
//!   D) virtual-workspace-glob — Workspace + glob member
//!   I) item-impl-methods — Symbol + DEFINES + HAS_PARENT

use assert_cmd::Command;
use serde_json::Value;
use std::path::PathBuf;

fn cli() -> Command {
    Command::cargo_bin("gitnexus-rust-core-cli").unwrap()
}

/// Rust-core workspace root（fixtures 所在位置）
fn rust_core_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// 运行 inspect --include graph 并返回 parsed GraphOutput
fn run_graph(fixture_rel: &str, include_symbols: bool) -> Value {
    let root = rust_core_root().join(fixture_rel);
    let mut cmd = cli();
    cmd.arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(&root)
        .arg("--include")
        .arg("graph");
    if include_symbols {
        cmd.arg("--include").arg("symbols");
    }
    let output = cmd.output().expect("failed to run CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("Graph output is not valid JSON")
}

/// 共享断言：每个 fixture 都必须通过
fn assert_common_graph_invariants(graph: &Value) {
    // 1. schemaVersion == "0.2.0"
    assert_eq!(graph["schemaVersion"], "0.2.0");

    // 2. generatedAt 非空
    let gen = graph["generatedAt"].as_str().unwrap();
    assert!(!gen.is_empty());

    // 3. stats.nodeCount > 0 && stats.edgeCount > 0
    assert!(graph["stats"]["nodeCount"].as_u64().unwrap() > 0);
    assert!(graph["stats"]["edgeCount"].as_u64().unwrap() > 0);

    // 4. 至少 1 个 Repository node
    let has_repo = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["label"] == "repository");
    assert!(has_repo, "missing Repository node");

    // 5. 至少 1 个 Package node
    let has_pkg = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["label"] == "package");
    assert!(has_pkg, "missing Package node");

    // 6. 至少 1 条 CONTAINS_PACKAGE 或 HAS_TARGET edge
    let has_structural = graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["type"] == "CONTAINS_PACKAGE" || e["type"] == "HAS_TARGET");
    assert!(
        has_structural,
        "missing CONTAINS_PACKAGE or HAS_TARGET edge"
    );

    // 7. 不存在 USES / IMPORTS / IMPLEMENTS edges（CALLS 需要 --include calls，这些 fixture 不触发）
    let forbidden = ["USES", "IMPORTS", "IMPLEMENTS"];
    for edge in graph["edges"].as_array().unwrap() {
        let etype = edge["type"].as_str().unwrap();
        assert!(
            !forbidden.contains(&etype),
            "forbidden edge type found: {etype}"
        );
    }

    // 8. Diagnostic ANNOTATES 边指向存在的 node id
    let node_ids: std::collections::HashSet<String> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    for edge in graph["edges"].as_array().unwrap() {
        if edge["type"] == "ANNOTATES" {
            let target = edge["target"].as_str().unwrap();
            assert!(
                node_ids.contains(target),
                "ANNOTATES edge points to non-existent node: {target}"
            );
        }
    }

    // 9. JSON 可 parse（已通过 serde_json::from_str 验证）
}

// ============================================================
// Fixture A: root-package
// ============================================================

#[test]
fn test_graph_root_package() {
    let graph = run_graph("fixtures/manifest-scanner/root-package", false);
    assert_common_graph_invariants(&graph);

    // 恰好 1 个 Repository、1 个 Package、1 个 Target（lib）
    let nodes = graph["nodes"].as_array().unwrap();
    let repo_count = nodes.iter().filter(|n| n["label"] == "repository").count();
    let pkg_count = nodes.iter().filter(|n| n["label"] == "package").count();
    let target_count = nodes.iter().filter(|n| n["label"] == "target").count();
    assert_eq!(repo_count, 1, "expected exactly 1 Repository");
    assert_eq!(pkg_count, 1, "expected exactly 1 Package");
    assert_eq!(target_count, 1, "expected exactly 1 Target (lib)");

    // 至少 1 个 SourceFile
    let sf_count = nodes.iter().filter(|n| n["label"] == "source-file").count();
    assert!(sf_count >= 1, "expected at least 1 SourceFile");

    // Target kind == lib
    let lib_targets: Vec<_> = nodes
        .iter()
        .filter(|n| n["label"] == "target" && n["properties"]["kind"] == "lib")
        .collect();
    assert_eq!(lib_targets.len(), 1, "expected 1 lib target");
}

#[test]
fn test_graph_root_package_deterministic() {
    let g1 = run_graph("fixtures/manifest-scanner/root-package", false);
    let g2 = run_graph("fixtures/manifest-scanner/root-package", false);
    let s1 = serde_json::to_string(&g1).unwrap();
    let s2 = serde_json::to_string(&g2).unwrap();
    assert_eq!(s1, s2, "graph output must be deterministic");
}

// ============================================================
// Fixture D: virtual-workspace-glob
// ============================================================

#[test]
fn test_graph_virtual_workspace_glob() {
    let graph = run_graph("fixtures/manifest-scanner/virtual-workspace-glob", false);
    assert_common_graph_invariants(&graph);

    let nodes = graph["nodes"].as_array().unwrap();

    // 至少 1 个 Workspace node
    let ws_count = nodes.iter().filter(|n| n["label"] == "workspace").count();
    assert!(ws_count >= 1, "expected at least 1 Workspace");

    // 至少 2 个 Package nodes（glob 展开）
    let pkg_count = nodes.iter().filter(|n| n["label"] == "package").count();
    assert!(
        pkg_count >= 2,
        "expected at least 2 Packages from glob, got {pkg_count}"
    );

    // CONTAINS_WORKSPACE edges count == workspace count
    let edges = graph["edges"].as_array().unwrap();
    let cw_count = edges
        .iter()
        .filter(|e| e["type"] == "CONTAINS_WORKSPACE")
        .count();
    assert_eq!(
        cw_count, ws_count,
        "CONTAINS_WORKSPACE count must match workspace count"
    );
}

#[test]
fn test_graph_virtual_workspace_glob_deterministic() {
    let g1 = run_graph("fixtures/manifest-scanner/virtual-workspace-glob", false);
    let g2 = run_graph("fixtures/manifest-scanner/virtual-workspace-glob", false);
    let s1 = serde_json::to_string(&g1).unwrap();
    let s2 = serde_json::to_string(&g2).unwrap();
    assert_eq!(s1, s2, "graph output must be deterministic");
}

// ============================================================
// Fixture I: item-impl-methods (with symbols)
// ============================================================

#[test]
fn test_graph_item_impl_methods() {
    let graph = run_graph("fixtures/item-extraction/item-impl-methods", true);
    assert_common_graph_invariants(&graph);

    let nodes = graph["nodes"].as_array().unwrap();
    let edges = graph["edges"].as_array().unwrap();

    // 至少 1 个 Symbol node
    let symbols: Vec<_> = nodes.iter().filter(|n| n["label"] == "symbol").collect();
    assert!(!symbols.is_empty(), "expected at least 1 Symbol node");

    // 至少 1 个 impl-block / method / associated-function
    let kinds: Vec<_> = symbols
        .iter()
        .map(|s| s["properties"]["symbolKind"].as_str().unwrap())
        .collect();
    assert!(
        kinds
            .iter()
            .any(|k| ["impl-block", "method", "associated-function"].contains(k)),
        "expected impl-block/method/associated-function, got: {kinds:?}"
    );

    // 至少 1 条 DEFINES edge
    let defines_count = edges.iter().filter(|e| e["type"] == "DEFINES").count();
    assert!(defines_count >= 1, "expected at least 1 DEFINES edge");

    // 至少 1 条 HAS_PARENT edge
    let parent_count = edges.iter().filter(|e| e["type"] == "HAS_PARENT").count();
    assert!(parent_count >= 1, "expected at least 1 HAS_PARENT edge");

    // Symbol 的 implDetails 非 null（impl-block 相关 symbol）
    let has_impl_details = symbols.iter().any(|s| {
        let kind = s["properties"]["symbolKind"].as_str().unwrap();
        matches!(kind, "impl-block" | "method" | "associated-function")
            && !s["properties"]["implDetails"].is_null()
    });
    assert!(
        has_impl_details,
        "expected at least 1 Symbol with non-null implDetails"
    );
}

#[test]
fn test_graph_item_impl_methods_deterministic() {
    let g1 = run_graph("fixtures/item-extraction/item-impl-methods", true);
    let g2 = run_graph("fixtures/item-extraction/item-impl-methods", true);
    let s1 = serde_json::to_string(&g1).unwrap();
    let s2 = serde_json::to_string(&g2).unwrap();
    assert_eq!(s1, s2, "graph output must be deterministic");
}

// ============================================================
// ============================================================
// v0.2 CALLS edge 验证
// ============================================================

/// 运行 graph inspect 并支持 custom args（如 --include calls）
fn run_graph_with_args(fixture_rel: &str, args: &[&str]) -> Value {
    let root = rust_core_root().join(fixture_rel);
    let mut cmd = cli();
    cmd.arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(&root);
    for arg in args {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("Graph output is not valid JSON")
}

#[test]
fn test_graph_c1_same_module_produces_calls_edges() {
    let graph = run_graph_with_args(
        "fixtures/call-resolution/c1-same-module",
        &[
            "--include",
            "graph",
            "--include",
            "calls",
            "--include",
            "symbols",
        ],
    );
    assert_common_graph_invariants(&graph);

    // 至少 1 条 CALLS edge
    let calls_edges: Vec<_> = graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "CALLS")
        .collect();
    assert!(!calls_edges.is_empty(), "expected at least 1 CALLS edge");

    // CALLS edge 属性完整
    let calls_edge = &calls_edges[0];
    assert_eq!(
        calls_edge["source"],
        "symbol:c1-same-module::crate::main_fn"
    );
    assert_eq!(calls_edge["target"], "symbol:c1-same-module::crate::helper");
    assert_eq!(calls_edge["properties"]["callKind"], "free-function");
    assert_eq!(
        calls_edge["properties"]["reason"],
        "call-same-module-resolved"
    );

    // stats.callEdgeCount > 0
    let call_edge_count = graph["stats"]["callEdgeCount"].as_u64().unwrap();
    assert!(
        call_edge_count > 0,
        "expected callEdgeCount > 0, got {call_edge_count}"
    );

    // 至少 2 个 Symbol node（caller + callee）
    let symbol_count = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["label"] == "symbol")
        .count();
    assert!(
        symbol_count >= 2,
        "expected at least 2 Symbol nodes, got {symbol_count}"
    );
}

#[test]
fn test_graph_c10_external_crate_produces_calls_edges_for_stdlib() {
    let graph = run_graph_with_args(
        "fixtures/call-resolution/c10-external-crate",
        &[
            "--include",
            "graph",
            "--include",
            "calls",
            "--include",
            "symbols",
        ],
    );
    assert_common_graph_invariants(&graph);

    // v0.2 + Phase 1 direct path resolution: stdlib calls (Vec/HashMap/PathBuf) 被解析
    // → CALLS edges 存在，但 third-party crate calls（如果有）不产 edge
    let calls_edges: Vec<_> = graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "CALLS")
        .collect();
    assert!(
        !calls_edges.is_empty(),
        "expected CALLS edges for stdlib calls, got 0"
    );

    // callEdgeCount > 0
    let call_edge_count = graph["stats"]["callEdgeCount"].as_u64().unwrap();
    assert!(
        call_edge_count > 0,
        "expected callEdgeCount > 0 for resolved stdlib calls"
    );
}

// ============================================================
// v0.2 Bug 0 fix: dangling CALLS edge（RISK_LEDGER §0）
// ============================================================

/// 验证 --include calls --include graph（无 --include symbols）时，
/// CALLS edge 的 source/target symbol node 存在。
/// 修复前：CALLS edges 指向不存在的 symbol node（dangling edge）。
/// 修复后：graph + calls 自动强制包含 symbols（edge endpoint integrity）。
#[test]
fn test_graph_calls_without_symbols_flag_preserves_endpoint_integrity() {
    let graph = run_graph_with_args(
        "fixtures/call-resolution/c1-same-module",
        &["--include", "graph", "--include", "calls"],
    );
    assert_common_graph_invariants(&graph);

    // 收集所有 node ID
    let node_ids: std::collections::HashSet<String> = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();

    // 每条 CALLS edge 的 source/target 必须存在于 nodes 中
    for edge in graph["edges"].as_array().unwrap() {
        if edge["type"] == "CALLS" {
            let source = edge["source"].as_str().unwrap();
            let target = edge["target"].as_str().unwrap();
            assert!(
                node_ids.contains(source),
                "CALLS edge source node missing: {source}"
            );
            assert!(
                node_ids.contains(target),
                "CALLS edge target node missing: {target}"
            );
        }
    }

    // Symbol nodes 必须存在（至少 caller + callee）
    let symbol_count = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["label"] == "symbol")
        .count();
    assert!(
        symbol_count >= 2,
        "expected at least 2 Symbol nodes (caller + callee) when --include calls --include graph, got {symbol_count}"
    );
}

// ============================================================
// Regression: --include graph 不影响不加 flag 的输出
// ============================================================

#[test]
fn test_graph_flag_does_not_affect_normal_output() {
    let root = rust_core_root().join("fixtures/manifest-scanner/root-package");
    let mut cmd_normal = cli();
    cmd_normal
        .arg("project-model")
        .arg("inspect")
        .arg("--root")
        .arg(&root);
    let normal = cmd_normal.output().expect("failed to run CLI");
    let normal_stdout = String::from_utf8_lossy(&normal.stdout);
    let normal_json: Value =
        serde_json::from_str(&normal_stdout).expect("normal output not valid JSON");

    // 正常输出不应有 schemaVersion (graph-only field)
    assert!(
        normal_json.get("schemaVersion").is_none(),
        "normal output should not contain graph schemaVersion"
    );
    // 正常输出应有 version field (ProjectModelOutput field)
    assert!(
        normal_json.get("version").is_some(),
        "normal output should contain version"
    );
}
