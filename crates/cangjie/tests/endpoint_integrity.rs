//! Integration tests for Cangjie graph endpoint integrity.
//!
//! Verifies that all edge source and target IDs reference existing nodes.
//! Also verifies: deterministic output, no duplicate nodes/edges, node kind coverage.
//!
//! Requires the `tree-sitter-cangjie` feature.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::{inspect_cangjie_project, NodeKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join("cjpm-basic")
}

fn imports_basic_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join("imports-basic")
}

fn constructor_basic_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join("constructor-basic")
}

#[test]
fn test_no_dangling_source_ids() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    // Collect all node IDs
    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // Check that all edge source IDs exist in nodes
    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.source_id.as_str()),
            "Edge source ID '{}' not found in nodes (target: '{}', kind: {:?})",
            edge.source_id,
            edge.target_id,
            edge.kind
        );
    }
}

#[test]
fn test_no_dangling_target_ids() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    // Collect all node IDs
    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // Check that all edge target IDs exist in nodes
    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.target_id.as_str()),
            "Edge target ID '{}' not found in nodes (source: '{}', kind: {:?})",
            edge.target_id,
            edge.source_id,
            edge.kind
        );
    }
}

#[test]
fn test_endpoint_integrity_on_imports_fixture() {
    let root = imports_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    let dangling_sources: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| !node_ids.contains(e.source_id.as_str()))
        .collect();

    assert!(
        dangling_sources.is_empty(),
        "Found {} dangling source IDs. Examples: {:?}",
        dangling_sources.len(),
        dangling_sources
            .iter()
            .take(3)
            .map(|e| &e.source_id)
            .collect::<Vec<_>>()
    );

    let dangling_targets: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| !node_ids.contains(e.target_id.as_str()))
        .collect();

    assert!(
        dangling_targets.is_empty(),
        "Found {} dangling target IDs. Examples: {:?}",
        dangling_targets.len(),
        dangling_targets
            .iter()
            .take(3)
            .map(|e| &e.target_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_endpoint_integrity_on_constructor_fixture() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.source_id.as_str()),
            "dangling source: '{}'",
            edge.source_id
        );
        assert!(
            node_ids.contains(edge.target_id.as_str()),
            "dangling target: '{}'",
            edge.target_id
        );
    }
}

#[test]
fn test_synthetic_nodes_are_marked() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    for node in &graph.nodes {
        if node.kind == NodeKind::CallableSource {
            assert_eq!(
                node.properties["synthetic"], true,
                "callableSource node '{}' should have synthetic=true, got: {:?}",
                node.id, node.properties
            );
            assert!(
                node.properties["kind"].is_string(),
                "callableSource node '{}' should have a kind property, got: {:?}",
                node.id,
                node.properties
            );
        }
    }
}

#[test]
fn test_no_duplicate_node_ids() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let mut seen_ids: HashSet<String> = HashSet::new();
    for node in &graph.nodes {
        assert!(
            seen_ids.insert(node.id.clone()),
            "duplicate node ID: '{}'",
            node.id
        );
    }
}

#[test]
fn test_node_id_determinism() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph1 = inspect_cangjie_project(&root).expect("inspect should succeed");
    let graph2 = inspect_cangjie_project(&root).expect("inspect should succeed");

    let ids1: Vec<_> = graph1.nodes.iter().map(|n| n.id.clone()).collect();
    let ids2: Vec<_> = graph2.nodes.iter().map(|n| n.id.clone()).collect();

    assert_eq!(ids1, ids2, "node IDs should be deterministic across runs");
}

#[test]
fn test_graph_output_determinism() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph1 = inspect_cangjie_project(&root).expect("inspect should succeed");
    let graph2 = inspect_cangjie_project(&root).expect("inspect should succeed");

    let json1 = serde_json::to_string_pretty(&graph1).unwrap();
    let json2 = serde_json::to_string_pretty(&graph2).unwrap();

    assert_eq!(json1, json2, "graph output should be deterministic");
}

#[test]
fn test_node_kind_coverage() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let kinds: HashSet<_> = graph.nodes.iter().map(|n| n.kind).collect();

    // 至少应有 Repository, Package, SourceFile, Symbol
    assert!(
        kinds.contains(&NodeKind::Repository),
        "missing Repository node"
    );
    assert!(kinds.contains(&NodeKind::Package), "missing Package node");
    assert!(
        kinds.contains(&NodeKind::SourceFile),
        "missing SourceFile node"
    );
    assert!(kinds.contains(&NodeKind::Symbol), "missing Symbol node");
}

#[test]
fn test_constructor_synthetic_nodes_zero_on_constructor_fixture() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // Constructor 类 synthetic nodes 应为 0（被真实 Init symbol 替代）
    let constructor_synthetic: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::CallableSource
                && n.properties
                    .get("kind")
                    .map_or(false, |v| v == "Constructor")
        })
        .collect();

    assert_eq!(
        constructor_synthetic.len(),
        0,
        "expected 0 Constructor synthetic nodes, got {}: {:?}",
        constructor_synthetic.len(),
        constructor_synthetic
            .iter()
            .map(|n| &n.id)
            .take(5)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_init_symbols_have_owner_name() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 所有 Init symbol 应有 owner_name（通过 label 格式 "Owner.init" 验证）
    let init_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .collect();

    for init in &init_nodes {
        assert!(
            init.label.contains(".init"),
            "Init symbol '{}' should have label 'Owner.init', got '{}'",
            init.id,
            init.label
        );
    }

    // 至少应有 AppConfig.init 和 Point.init
    assert!(
        init_nodes.len() >= 2,
        "expected at least 2 Init symbols, got {}",
        init_nodes.len()
    );
}
