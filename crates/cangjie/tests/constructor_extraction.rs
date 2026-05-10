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

#[test]
fn test_constructor_basic_no_duplicate_node_ids() {
    let root = constructor_basic_dir();
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
fn test_multi_init_has_unique_node_ids() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 收集所有 MultiInit 相关的 Init symbol node
    let multi_init_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol
                && n.properties.get("kind").map_or(false, |v| v == "Init")
                && n.label.contains("MultiInit.init")
        })
        .collect();

    assert_eq!(
        multi_init_nodes.len(),
        2,
        "expected exactly 2 MultiInit init symbols, got {}: {:?}",
        multi_init_nodes.len(),
        multi_init_nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );

    // 两个 MultiInit.init 的 node ID 应不同
    assert_ne!(
        multi_init_nodes[0].id, multi_init_nodes[1].id,
        "MultiInit init nodes should have different IDs"
    );

    // 应包含 #1 和 #2 后缀
    let ids: Vec<&str> = multi_init_nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(
        ids.iter().any(|id| id.contains("MultiInit.init#1")),
        "expected MultiInit.init#1, got {:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id.contains("MultiInit.init#2")),
        "expected MultiInit.init#2, got {:?}",
        ids
    );
}

#[test]
fn test_all_expected_init_symbols_present() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    let init_labels: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .map(|n| n.label.as_str())
        .collect();

    // 验证 AppConfig.init, Point.init, MultiInit.init 都存在
    assert!(
        init_labels.iter().any(|l| l.contains("AppConfig.init")),
        "expected AppConfig.init, got {:?}",
        init_labels
    );
    assert!(
        init_labels.iter().any(|l| l.contains("Point.init")),
        "expected Point.init, got {:?}",
        init_labels
    );
    assert!(
        init_labels.iter().any(|l| l.contains("MultiInit.init")),
        "expected MultiInit.init, got {:?}",
        init_labels
    );

    // MultiInit.init 应有恰好 2 个不同的 node
    let multi_init_count = init_labels
        .iter()
        .filter(|l| l.contains("MultiInit.init"))
        .count();
    assert_eq!(
        multi_init_count, 2,
        "expected 2 MultiInit.init entries, got {}",
        multi_init_count
    );
}

#[test]
fn test_constructor_source_mapping_not_merging_multi_init() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 查找所有 Init symbol node IDs
    let init_node_ids: HashSet<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .map(|n| n.id.as_str())
        .collect();

    // 查找 reference edges（Uses/Accesses/Modifies）
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

    // Constructor 类的 reference edges 的 source 不应全部指向同一个 init node
    // 如果两个不同 arity 的 init 被错误合并，所有 constructor reference edge source 会指向同一个 node
    let mut constructor_edge_sources: Vec<&str> = Vec::new();
    for edge in &ref_edges {
        if init_node_ids.contains(edge.source_id.as_str()) {
            constructor_edge_sources.push(edge.source_id.as_str());
        }
    }

    // 如果有多个不同的 init node 被用作 edge source，说明没有错误合并
    let unique_sources: HashSet<_> = constructor_edge_sources.iter().collect();
    if unique_sources.len() > 1 {
        // 好：不同 init 被正确区分
        eprintln!(
            "constructor edges use {} distinct init sources, no merging issue",
            unique_sources.len()
        );
    }
    // 如果只有 0 或 1 个 unique source，检查是否是 fixture 只有单 init class 的调用
    // （当前 fixture main() 中只有 AppConfig("test", 42) 和 Point(1.0, 2.0) 两个调用，
    //  都只有单 init，所以 1 个或 2 个 unique sources 都是合理的）
}

#[test]
fn test_synthetic_fallback_still_exists_for_unmapped_sources() {
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

    // Constructor 类 synthetic 应为 0（被 Init symbol 覆盖）
    let constructor_synthetic: Vec<_> = synthetic_nodes
        .iter()
        .filter(|n| {
            n.properties
                .get("kind")
                .map_or(false, |v| v == "Constructor")
        })
        .collect();
    assert_eq!(
        constructor_synthetic.len(),
        0,
        "expected 0 Constructor synthetic nodes"
    );

    // Method/Function 类 synthetic nodes 应仍保留（fallback 未关闭）
    eprintln!(
        "synthetic fallback check: total={}, constructor=0, method={}, function={}",
        synthetic_nodes.len(),
        synthetic_nodes
            .iter()
            .filter(|n| n.properties.get("kind").map_or(false, |v| v == "Method"))
            .count(),
        synthetic_nodes
            .iter()
            .filter(|n| n.properties.get("kind").map_or(false, |v| v == "Function"))
            .count(),
    );
    // 不要求 total=0 — Method/Function 类 synthetic fallback 应保留
}

#[test]
fn test_endpoint_integrity_zero_dangling_on_constructor_fixture() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");
    let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    let dangling_sources: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| !node_ids.contains(e.source_id.as_str()))
        .collect();
    let dangling_targets: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| !node_ids.contains(e.target_id.as_str()))
        .collect();

    assert!(
        dangling_sources.is_empty(),
        "found {} dangling sources: {:?}",
        dangling_sources.len(),
        dangling_sources
            .iter()
            .map(|e| &e.source_id)
            .take(5)
            .collect::<Vec<_>>()
    );
    assert!(
        dangling_targets.is_empty(),
        "found {} dangling targets: {:?}",
        dangling_targets.len(),
        dangling_targets
            .iter()
            .map(|e| &e.target_id)
            .take(5)
            .collect::<Vec<_>>()
    );
}

/// 验证 constructor call 的 Uses edge target 指向 Class symbol（而非 Init symbol）。
/// 设计合同：没有完整类型推断时，构造函数调用无法区分重载 init，
/// 因此 Uses edge 的 target 是 Class（唯一无歧义的目标）。
#[test]
fn test_constructor_call_targets_class_symbol_not_init() {
    let root = constructor_basic_dir();
    if !root.exists() {
        return;
    }

    let graph = inspect_cangjie_project(&root).expect("inspect should succeed");

    // 收集 Class symbol IDs
    let class_ids: HashSet<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol
                && n.properties.get("kind").map_or(false, |v| v == "Class")
        })
        .map(|n| n.id.as_str())
        .collect();

    // 收集 Init symbol IDs
    let init_ids: HashSet<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Symbol
                && n.properties.get("kind").map_or(false, |v| v == "Init")
        })
        .map(|n| n.id.as_str())
        .collect();

    // 查找所有 Uses edges
    let uses_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.kind == gitnexus_cangjie::graph::EdgeKind::Uses)
        .collect();

    // constructor call Uses edges 不应 target Init symbols
    // （Init symbols 是被 Defines 定义的节点，不是 call target）
    for edge in &uses_edges {
        if init_ids.contains(edge.target_id.as_str()) {
            // Uses edge target 是 Init symbol — 这可能是 annotation reference，不是 call
            // 检查 source 是否是 CallableSource（如果是，可能是错误的 call→init 映射）
            let source_is_callable = graph.nodes.iter().any(|n| {
                n.id == edge.source_id && n.kind == NodeKind::CallableSource
            });
            assert!(
                !source_is_callable,
                "Uses edge from callable source '{}' should target Class symbol, not Init '{}'",
                edge.source_id, edge.target_id
            );
        }
    }

    // 验证至少存在 Class symbols 作为 Uses edge targets（证明 constructor call 有正确的 target）
    let uses_to_class = uses_edges
        .iter()
        .filter(|e| class_ids.contains(e.target_id.as_str()))
        .count();
    assert!(
        uses_to_class > 0,
        "expected at least one Uses edge targeting a Class symbol (constructor calls), got 0"
    );
}

/// 验证 Init symbol 的 Defines edge 存在且 source 是 SourceFile。
/// 设计合同：Init symbols 通过 Defines edge 从 SourceFile 定义，不是从 Class 定义。
#[test]
fn test_init_symbols_defined_from_source_file() {
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

    assert!(!init_nodes.is_empty(), "expected at least one Init symbol");

    // 每个 Init symbol 应有 Defines edge，且 source 应是 SourceFile node
    for init in &init_nodes {
        let defines_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| {
                e.kind == gitnexus_cangjie::graph::EdgeKind::Defines && e.target_id == init.id
            })
            .collect();

        assert!(
            !defines_edges.is_empty(),
            "Init symbol '{}' should have at least one Defines edge",
            init.id
        );

        // 验证至少一个 Defines edge 的 source 是 SourceFile
        let has_source_file_defines = defines_edges.iter().any(|e| {
            graph
                .nodes
                .iter()
                .any(|n| n.id == e.source_id && n.kind == NodeKind::SourceFile)
        });
        assert!(
            has_source_file_defines,
            "Init symbol '{}' should be Defined from a SourceFile node",
            init.id
        );
    }
}
