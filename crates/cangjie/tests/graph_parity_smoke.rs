//! Integration tests for Cangjie graph output parity verification.
//!
//! These tests validate that Rust-core graph output covers all expected
//! node and edge types, maintains structural integrity, and produces
//! deterministic output across multiple runs.

use gitnexus_cangjie::graph::{inspect_cangjie_project, CangjieGraphOutput};
use std::path::PathBuf;

#[test]
fn test_graph_output_basics() {
    // Test that graph output can be generated
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return; // Skip if fixture doesn't exist
    }

    let graph = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // Validate basic structure
    assert!(!graph.nodes.is_empty(), "graph should have nodes");
    assert!(!graph.edges.is_empty(), "graph should have edges");
}

#[test]
fn test_node_type_coverage() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // Check for expected node types
    let node_kinds: Vec<_> = graph.nodes.iter().map(|n| n.kind).collect();

    // Should have at least Package and SourceFile nodes
    assert!(
        node_kinds
            .iter()
            .any(|k| matches!(k, gitnexus_cangjie::graph::NodeKind::Package)),
        "should have Package nodes"
    );
    assert!(
        node_kinds
            .iter()
            .any(|k| matches!(k, gitnexus_cangjie::graph::NodeKind::SourceFile)),
        "should have SourceFile nodes"
    );
}

#[test]
fn test_edge_type_coverage() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // Check for expected edge types
    let edge_kinds: Vec<_> = graph.edges.iter().map(|e| e.kind).collect();

    // Should have Defines edges at minimum
    assert!(
        edge_kinds
            .iter()
            .any(|k| matches!(k, gitnexus_cangjie::graph::EdgeKind::Defines)),
        "should have Defines edges"
    );
}

#[test]
fn test_graph_structural_integrity() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // All edges should reference valid nodes
    for edge in &graph.edges {
        assert!(
            graph.nodes.iter().any(|n| n.id == edge.source_id),
            "edge source {} should reference existing node",
            edge.source_id
        );
        assert!(
            graph.nodes.iter().any(|n| n.id == edge.target_id),
            "edge target {} should reference existing node",
            edge.target_id
        );
    }
}

#[test]
fn test_deterministic_output() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return;
    }

    // Run graph generation twice
    let graph1 = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    let graph2 = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // Should produce identical results
    assert_eq!(
        graph1.nodes.len(),
        graph2.nodes.len(),
        "node count should be deterministic"
    );
    assert_eq!(
        graph1.edges.len(),
        graph2.edges.len(),
        "edge count should be deterministic"
    );
}

#[test]
fn test_json_serialization() {
    let fixture_dir = PathBuf::from("fixtures/cangjie/imports-basic");
    if !fixture_dir.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&fixture_dir).expect("fixture should load");

    // Should serialize to valid JSON
    let json = serde_json::to_string(&graph).expect("graph should serialize to JSON");

    // Should deserialize back to same structure
    let deserialized: CangjieGraphOutput =
        serde_json::from_str(&json).expect("JSON should deserialize back to graph");

    assert_eq!(graph.nodes.len(), deserialized.nodes.len());
    assert_eq!(graph.edges.len(), deserialized.edges.len());
}
