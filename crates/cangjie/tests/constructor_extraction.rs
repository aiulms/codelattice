//! Integration tests for Cangjie constructor symbol extraction and graph emission.
//!
//! Verifies:
//! - Init symbols are correctly extracted from class/struct definitions
//! - Constructor source IDs are mapped to init symbol node IDs
//! - Synthetic nodes are reduced (only Method/Function fallback remains)
//! - Endpoint integrity is maintained (0 dangling)
//!
//! Requires the `tree-sitter-cangjie` feature.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::{inspect_cangjie_project, NodeKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

fn constructor_cross_file_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join("constructor-cross-file")
}

#[test]
fn test_constructor_basic_init_symbols_extracted() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 查找 Init 类型的 Symbol nodes
    let init_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .collect();

    // 预期至少有 AppConfig.init, Point.init, MultiInit.init (x2)
    assert!(
        !init_nodes.is_empty(),
        "expected at least one Init symbol node, got {} nodes total",
        graph.nodes.len()
    );

    // 验证 init nodes 有正确的 label
    let init_labels: Vec<_> = init_nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(
        init_labels.iter().any(|l| l.contains("AppConfig.init")),
        "expected AppConfig.init in {:?}",
        init_labels
    );
    assert!(
        init_labels.iter().any(|l| l.contains("Point.init")),
        "expected Point.init in {:?}",
        init_labels
    );
}

#[test]
fn test_constructor_basic_endpoint_integrity() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // 验证所有 edge source IDs 存在于 nodes
    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.source_id.as_str()),
            "dangling source: '{}' (kind: {:?}, target: '{}')",
            edge.source_id,
            edge.kind,
            edge.target_id
        );
    }

    // 验证所有 edge target IDs 存在于 nodes
    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.target_id.as_str()),
            "dangling target: '{}' (kind: {:?}, source: '{}')",
            edge.target_id,
            edge.kind,
            edge.source_id
        );
    }
}

#[test]
fn test_constructor_basic_synthetic_nodes_reduced() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 统计 synthetic nodes
    let synthetic_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CallableSource)
        .collect();

    // 统计 Constructor 类 synthetic nodes
    let constructor_synthetic: Vec<_> = synthetic_nodes
        .iter()
        .filter(|n| {
            n.properties
                .get("kind")
                .map_or(false, |v| v == "Constructor")
        })
        .collect();

    // Constructor 类 synthetic nodes 应为 0（被真实 Init symbol 替代）
    assert_eq!(
        constructor_synthetic.len(),
        0,
        "expected 0 Constructor synthetic nodes (covered by Init symbols), got {}",
        constructor_synthetic.len()
    );

    // 记录 synthetic nodes 总数（可能有 Method/Function 类作为 fallback）
    eprintln!(
        "synthetic nodes: total={}, constructor={}, method={}, function={}",
        synthetic_nodes.len(),
        constructor_synthetic.len(),
        synthetic_nodes
            .iter()
            .filter(|n| n.properties.get("kind").map_or(false, |v| v == "Method"))
            .count(),
        synthetic_nodes
            .iter()
            .filter(|n| n.properties.get("kind").map_or(false, |v| v == "Function"))
            .count(),
    );
}

#[test]
fn test_constructor_basic_init_has_defines_edge() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 查找 Init symbol nodes
    let init_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .collect();

    // 每个 Init symbol 应有对应的 Defines edge
    for init_node in &init_nodes {
        let has_defines = graph.edges.iter().any(|e| {
            e.kind == gitnexus_cangjie::graph::EdgeKind::Defines && e.target_id == init_node.id
        });
        assert!(
            has_defines,
            "Init symbol '{}' should have a Defines edge",
            init_node.id
        );
    }
}

#[test]
fn test_constructor_cross_file_endpoint_integrity() {
    let root = constructor_cross_file_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    for edge in &graph.edges {
        assert!(
            node_ids.contains(edge.source_id.as_str()),
            "dangling source: '{}' (kind: {:?})",
            edge.source_id,
            edge.kind
        );
        assert!(
            node_ids.contains(edge.target_id.as_str()),
            "dangling target: '{}' (kind: {:?})",
            edge.target_id,
            edge.kind
        );
    }
}

#[test]
fn test_constructor_basic_reference_edges_use_init_symbol_as_source() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 查找 init symbol node IDs
    let init_node_ids: HashSet<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .map(|n| n.id.as_str())
        .collect();

    // 查找 Constructor synthetic node IDs
    let constructor_synthetic_ids: HashSet<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::CallableSource
                && n.properties
                    .get("kind")
                    .map_or(false, |v| v == "Constructor")
        })
        .map(|n| n.id.as_str())
        .collect();

    // 查找 Uses/Accesses/Modifies edges
    let ref_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                gitnexus_cangjie::graph::EdgeKind::Uses
                    | gitnexus_cangjie::graph::EdgeKind::Accesses
                    | gitnexus_cangjie::graph::EdgeKind::Modifies
            )
        })
        .collect();

    // 对于 Constructor 类的 reference edges，source 应指向 init symbol 而非 synthetic node
    for edge in &ref_edges {
        // 如果 source 是 init symbol，正确
        if init_node_ids.contains(edge.source_id.as_str()) {
            continue;
        }
        // 如果 source 是 Constructor synthetic node，说明映射未生效
        assert!(
            !constructor_synthetic_ids.contains(edge.source_id.as_str()),
            "reference edge source '{}' should be mapped to init symbol, not synthetic Constructor node",
            edge.source_id
        );
    }
}
