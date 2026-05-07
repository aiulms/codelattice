//! Integration tests for Cangjie graph endpoint integrity.
//!
//! Verifies that all edge source and target IDs reference existing nodes.
//! This is critical to prevent dangling edges in the graph output.
//!
//! Requires the `tree-sitter-cangjie` feature.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::inspect_cangjie_project;
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
fn test_endpoint_integrity_on_production_fixture() {
    // This test verifies endpoint integrity on a larger fixture
    // It should pass after Slice 19 (synthetic source nodes)
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join("imports-basic");

    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    // Collect all node IDs
    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // Check for any dangling source IDs
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

    // Check for any dangling target IDs
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
fn test_synthetic_nodes_are_marked() {
    let root = fixture_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

    // Verify that all synthetic nodes are properly marked
    for node in &graph.nodes {
        if node.kind == gitnexus_cangjie::graph::NodeKind::CallableSource {
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
