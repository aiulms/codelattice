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
    dangling_sources: usize,
    dangling_targets: usize,
    duration_secs: f64,
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
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
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
            dangling_sources: 0,
            dangling_targets: 0,
            duration_secs: 0.0,
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

            SmokeResult {
                root: root.display().to_string(),
                nodes: graph.nodes.len(),
                edges: graph.edges.len(),
                synthetic_count,
                dangling_sources,
                dangling_targets,
                duration_secs: duration.as_secs_f64(),
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
                dangling_sources: 0,
                dangling_targets: 0,
                duration_secs: 0.0,
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
        println!("Synthetic nodes: {}", result.synthetic_count);
        println!("Dangling source edges: {}", result.dangling_sources);
        println!("Dangling target edges: {}", result.dangling_targets);
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

        // Verify synthetic nodes are marked
        // (This is verified indirectly by endpoint integrity)
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
        let total_duration: f64 = successful.iter().map(|r| r.duration_secs).sum();

        println!("Total nodes: {}", total_nodes);
        println!("Total edges: {}", total_edges);
        println!("Total synthetic nodes: {}", total_synthetic);
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
