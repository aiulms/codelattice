//! Multi-project production smoke tests for Cangjie graph output.
//!
//! Verifies that synthetic source nodes work correctly across multiple
//! real Cangjie projects, ensuring endpoint integrity and output determinism.
//!
//! Requires the `tree-sitter-cangjie` feature.
//!
//! **Opt-in**: 此测试依赖本机绝对路径，默认 #[ignore]。
//! 手动 production smoke 命令：
//! ```sh
//! cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
//! ```

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::inspect_cangjie_project;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// Result of smoking a single Cangjie project.
#[derive(Debug, Clone)]
struct SmokeResult {
    root: String,
    nodes: usize,
    edges: usize,
    synthetic_count: usize,
    synthetic_constructor: usize,
    synthetic_method: usize,
    synthetic_function: usize,
    init_symbol_count: usize,
    init_with_arity: usize,
    duplicate_nodes: usize,
    duplicate_edges: usize,
    dangling_sources: usize,
    dangling_targets: usize,
    duration_secs: f64,
    deterministic: bool,
    node_kind_distribution: HashMap<String, usize>,
    edge_kind_distribution: HashMap<String, usize>,
    skipped: bool,
    skip_reason: Option<String>,
}

/// Run smoke test on a single Cangjie project.
fn run_smoke(root: &Path) -> SmokeResult {
    // Check if path exists
    if !root.exists() {
        return SmokeResult {
            root: root.display().to_string(),
            nodes: 0,
            edges: 0,
            synthetic_count: 0,
            synthetic_constructor: 0,
            synthetic_method: 0,
            synthetic_function: 0,
            init_symbol_count: 0,
            init_with_arity: 0,
            duplicate_nodes: 0,
            duplicate_edges: 0,
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
            deterministic: false,
            node_kind_distribution: HashMap::new(),
            edge_kind_distribution: HashMap::new(),
            skipped: true,
            skip_reason: Some("Path does not exist".to_string()),
        };
    }

    // Check if cjpm.toml exists
    let cjpm_toml = root.join("cjpm.toml");
    if !cjpm_toml.exists() {
        return SmokeResult {
            root: root.display().to_string(),
            nodes: 0,
            edges: 0,
            synthetic_count: 0,
            synthetic_constructor: 0,
            synthetic_method: 0,
            synthetic_function: 0,
            init_symbol_count: 0,
            init_with_arity: 0,
            duplicate_nodes: 0,
            duplicate_edges: 0,
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
            deterministic: false,
            node_kind_distribution: HashMap::new(),
            edge_kind_distribution: HashMap::new(),
            skipped: true,
            skip_reason: Some("cjpm.toml not found".to_string()),
        };
    }

    let start = Instant::now();
    match inspect_cangjie_project(root) {
        Ok(graph) => {
            let duration = start.elapsed();

            // Collect node IDs
            let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

            // Check endpoint integrity
            let dangling_sources = graph
                .edges
                .iter()
                .filter(|e| !node_ids.contains(e.source_id.as_str()))
                .count();

            let dangling_targets = graph
                .edges
                .iter()
                .filter(|e| !node_ids.contains(e.target_id.as_str()))
                .count();

            // Count synthetic nodes
            let synthetic_count = graph
                .nodes
                .iter()
                .filter(|n| n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource)
                .count();

            // Build node kind distribution
            let mut node_kind_distribution = HashMap::new();
            for node in &graph.nodes {
                let kind_str = format!("{:?}", node.kind);
                *node_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

            // Build edge kind distribution
            let mut edge_kind_distribution = HashMap::new();
            for edge in &graph.edges {
                let kind_str = format!("{:?}", edge.kind);
                *edge_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

            // Breakdown synthetic nodes by kind (Constructor/Method/Function)
            let mut synthetic_constructor = 0usize;
            let mut synthetic_method = 0usize;
            let mut synthetic_function = 0usize;
            for node in &graph.nodes {
                if node.kind == gitnexus_cangjie::graph::NodeKind::CallableSource {
                    if let Some(kind_val) = node.properties.get("kind") {
                        match kind_val.as_str().unwrap_or("") {
                            "Constructor" => synthetic_constructor += 1,
                            "Method" => synthetic_method += 1,
                            "Function" => synthetic_function += 1,
                            _ => {}
                        }
                    }
                }
            }

            // Count Init symbols and verify #arity suffix
            let mut init_symbol_count = 0usize;
            let mut init_with_arity = 0usize;
            for node in &graph.nodes {
                if node.kind == gitnexus_cangjie::graph::NodeKind::Symbol {
                    if let Some(kind_val) = node.properties.get("kind") {
                        if kind_val.as_str() == Some("Init") {
                            init_symbol_count += 1;
                            if node.id.contains('#') {
                                init_with_arity += 1;
                            }
                        }
                    }
                }
            }

            // Check duplicate node IDs
            let mut seen_node_ids: HashSet<&str> = HashSet::new();
            let mut duplicate_nodes = 0usize;
            for node in &graph.nodes {
                if !seen_node_ids.insert(node.id.as_str()) {
                    duplicate_nodes += 1;
                }
            }

            // Check duplicate edge triples
            let mut seen_edge_triples: HashSet<(String, String, String)> = HashSet::new();
            let mut duplicate_edges = 0usize;
            for edge in &graph.edges {
                let triple = (
                    format!("{:?}", edge.kind),
                    edge.source_id.clone(),
                    edge.target_id.clone(),
                );
                if !seen_edge_triples.insert(triple) {
                    duplicate_edges += 1;
                }
            }

            // Output determinism: run a second time and compare JSON
            let deterministic = match inspect_cangjie_project(root) {
                Ok(graph2) => {
                    let json1 = serde_json::to_value(&graph).unwrap_or_default();
                    let json2 = serde_json::to_value(&graph2).unwrap_or_default();
                    json1 == json2
                }
                Err(_) => false,
            };

            SmokeResult {
                root: root.display().to_string(),
                nodes: graph.nodes.len(),
                edges: graph.edges.len(),
                synthetic_count,
                synthetic_constructor,
                synthetic_method,
                synthetic_function,
                init_symbol_count,
                init_with_arity,
                duplicate_nodes,
                duplicate_edges,
                dangling_sources,
                dangling_targets,
                duration_secs: duration.as_secs_f64(),
                deterministic,
                node_kind_distribution,
                edge_kind_distribution,
                skipped: false,
                skip_reason: None,
            }
        }
        Err(e) => {
            eprintln!("Failed to inspect {}: {}", root.display(), e);
            SmokeResult {
                root: root.display().to_string(),
                nodes: 0,
                edges: 0,
                synthetic_count: 0,
                synthetic_constructor: 0,
                synthetic_method: 0,
                synthetic_function: 0,
                init_symbol_count: 0,
                init_with_arity: 0,
                duplicate_nodes: 0,
                duplicate_edges: 0,
                dangling_sources: 0,
                dangling_targets: 0,
                duration_secs: 0.0,
                deterministic: false,
                node_kind_distribution: HashMap::new(),
                edge_kind_distribution: HashMap::new(),
                skipped: true,
                skip_reason: Some(format!("Error: {}", e)),
            }
        }
    }
}

#[test]
#[ignore] // 依赖本机绝对路径，默认跳过；手动 opt-in: --ignored
fn test_multi_project_smoke_with_details() {
    // Define smoke targets (read-only access)
    let targets = vec![
        Path::new("/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui"),
        Path::new("/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui"),
        Path::new(
            "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project",
        ),
        Path::new(
            "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project",
        ),
    ];

    let mut results = Vec::new();

    for root in targets {
        let result = run_smoke(root);
        results.push(result.clone());

        if result.skipped {
            println!("\n=== Skipped: {} ===", result.root);
            println!("Reason: {}", result.skip_reason.unwrap_or_default());
            continue;
        }

        println!("\n=== Project: {} ===", result.root);
        println!("Nodes: {}", result.nodes);
        println!("Edges: {}", result.edges);
        println!(
            "Synthetic nodes: total={}, Constructor={}, Method={}, Function={}",
            result.synthetic_count,
            result.synthetic_constructor,
            result.synthetic_method,
            result.synthetic_function
        );
        println!(
            "Init symbols: total={}, with_arity={}",
            result.init_symbol_count, result.init_with_arity
        );
        println!("Duplicate node IDs: {}", result.duplicate_nodes);
        println!("Duplicate edge triples: {}", result.duplicate_edges);
        println!("Dangling source edges: {}", result.dangling_sources);
        println!("Dangling target edges: {}", result.dangling_targets);
        println!("Output deterministic: {}", result.deterministic);
        println!("Duration: {:.3}s", result.duration_secs);
        println!(
            "Node kind distribution: {:?}",
            result.node_kind_distribution
        );
        println!(
            "Edge kind distribution: {:?}",
            result.edge_kind_distribution
        );

        // Assert endpoint integrity
        assert_eq!(
            result.dangling_sources, 0,
            "Dangling source edges found in {}",
            result.root
        );
        assert_eq!(
            result.dangling_targets, 0,
            "Dangling target edges found in {}",
            result.root
        );

        // Assert no duplicate node IDs (graph identity)
        assert_eq!(
            result.duplicate_nodes, 0,
            "Duplicate node IDs found in {}",
            result.root
        );

        // Assert no duplicate edge triples (graph identity, post-deduplication)
        assert_eq!(
            result.duplicate_edges, 0,
            "Duplicate edge triples found in {}",
            result.root
        );

        // Assert output determinism
        assert!(
            result.deterministic,
            "Output not deterministic for {}",
            result.root
        );

        // Assert all Init symbols have #arity suffix
        if result.init_symbol_count > 0 {
            assert_eq!(
                result.init_symbol_count, result.init_with_arity,
                "Not all Init symbols have #arity suffix in {}: {}/{}",
                result.root, result.init_with_arity, result.init_symbol_count
            );
        }

        // Assert Constructor synthetic nodes are eliminated by Init symbols
        assert_eq!(
            result.synthetic_constructor, 0,
            "Constructor synthetic nodes should be 0 (covered by Init symbols) in {}, got {}",
            result.root, result.synthetic_constructor
        );
    }

    // Print summary
    let successful: Vec<_> = results.iter().filter(|r| !r.skipped).collect();
    let skipped: Vec<_> = results.iter().filter(|r| r.skipped).collect();

    println!("\n=== Summary ===");
    println!("Total targets: {}", results.len());
    println!("Successfully smoked: {}", successful.len());
    println!("Skipped: {}", skipped.len());

    if !successful.is_empty() {
        let total_nodes: usize = successful.iter().map(|r| r.nodes).sum();
        let total_edges: usize = successful.iter().map(|r| r.edges).sum();
        let total_synthetic: usize = successful.iter().map(|r| r.synthetic_count).sum();
        let total_synthetic_constructor: usize =
            successful.iter().map(|r| r.synthetic_constructor).sum();
        let total_synthetic_method: usize = successful.iter().map(|r| r.synthetic_method).sum();
        let total_synthetic_function: usize = successful.iter().map(|r| r.synthetic_function).sum();
        let total_init: usize = successful.iter().map(|r| r.init_symbol_count).sum();
        let total_duration: f64 = successful.iter().map(|r| r.duration_secs).sum();

        println!("Total nodes: {}", total_nodes);
        println!("Total edges: {}", total_edges);
        println!(
            "Total synthetic nodes: {} (Constructor={}, Method={}, Function={})",
            total_synthetic,
            total_synthetic_constructor,
            total_synthetic_method,
            total_synthetic_function
        );
        println!("Total Init symbols: {}", total_init);
        println!("Total duration: {:.3}s", total_duration);
    }

    if !skipped.is_empty() {
        println!("\nSkipped projects:");
        for result in skipped {
            println!(
                "- {} ({})",
                result.root,
                result
                    .skip_reason
                    .as_ref()
                    .unwrap_or(&"Unknown".to_string())
            );
        }
    }

    // Assert at least 3 successful targets
    assert!(
        successful.len() >= 3,
        "Expected at least 3 successful targets, got {}",
        successful.len()
    );
}
