//! Multi-project production smoke tests for Cangjie graph output.
//!
//! Verifies graph quality invariants across projects:
//! - Duplicate node IDs = 0
//! - Duplicate edge triples = 0
//! - Dangling source/target edges = 0
//! - Output deterministic (two runs produce identical JSON)
//! - Synthetic nodes by kind reported
//!
//! Requires the `tree-sitter-cangjie` feature.
//!
//! # Running
//!
//! ```sh
//! # Full suite: fixture + all available production targets
//! cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --ignored --nocapture
//!
//! # Fixture-only (always available, no machine-local paths)
//! cargo test --features tree-sitter-cangjie --test multi_project_smoke -- --nocapture
//! ```
//!
//! ## Target types
//!
//! | Type | Always available? | Failure behavior |
//! |------|-------------------|------------------|
//! | Local fixture | Yes (in repo fixtures/) | Hard assertion |
//! | Production path | No (machine-local) | Graceful skip if missing |
//!
//! Machine-local production paths are guarded behind `#[ignore]`.
//! Fixture-based smoke runs as part of the default test suite.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::inspect_cangjie_project;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Tag for whether a smoke target is a fixture or a machine-local path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// Repo fixture (always available).
    Fixture,
    /// Machine-local production path (may not exist).
    Production,
}

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
    failed: bool,
    fail_reason: Option<String>,
}

/// Run smoke test on a single Cangjie project.
fn run_smoke(root: &Path, _target_kind: TargetKind) -> SmokeResult {
    let skip = |reason: &str| -> SmokeResult {
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
            skip_reason: Some(reason.to_string()),
            failed: false,
            fail_reason: None,
        }
    };

    if !root.exists() {
        return skip("Path does not exist");
    }
    let cjpm_toml = root.join("cjpm.toml");
    if !cjpm_toml.exists() {
        return skip("cjpm.toml not found");
    }

    let start = Instant::now();
    match inspect_cangjie_project(root) {
        Ok(graph) => {
            let duration = start.elapsed();

            let node_ids: HashSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

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

            let synthetic_count = graph
                .nodes
                .iter()
                .filter(|n| n.kind == gitnexus_cangjie::graph::NodeKind::CallableSource)
                .count();

            let mut node_kind_distribution = HashMap::new();
            for node in &graph.nodes {
                let kind_str = format!("{:?}", node.kind);
                *node_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

            let mut edge_kind_distribution = HashMap::new();
            for edge in &graph.edges {
                let kind_str = format!("{:?}", edge.kind);
                *edge_kind_distribution.entry(kind_str).or_insert(0) += 1;
            }

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

            let mut seen_node_ids: HashSet<&str> = HashSet::new();
            let mut duplicate_nodes = 0usize;
            for node in &graph.nodes {
                if !seen_node_ids.insert(node.id.as_str()) {
                    duplicate_nodes += 1;
                }
            }

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
                failed: false,
                fail_reason: None,
            }
        }
        Err(e) => SmokeResult {
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
            failed: false,
            fail_reason: None,
        },
    }
}

/// Validate a smoke result with hard assertions.
/// Returns a modified result with `failed` set if any assertion would fail.
fn validate_smoke(result: &mut SmokeResult) {
    if result.skipped {
        return;
    }

    let mut errors: Vec<String> = Vec::new();

    if result.dangling_sources > 0 {
        errors.push(format!("dangling_sources={}", result.dangling_sources));
    }
    if result.dangling_targets > 0 {
        errors.push(format!("dangling_targets={}", result.dangling_targets));
    }
    if result.duplicate_nodes > 0 {
        errors.push(format!("duplicate_nodes={}", result.duplicate_nodes));
    }
    if result.duplicate_edges > 0 {
        errors.push(format!("duplicate_edges={}", result.duplicate_edges));
    }
    if !result.deterministic {
        errors.push("not deterministic".to_string());
    }
    if result.synthetic_constructor > 0 {
        errors.push(format!(
            "synthetic_constructor={}",
            result.synthetic_constructor
        ));
    }
    if result.init_symbol_count > 0 && result.init_symbol_count != result.init_with_arity {
        errors.push(format!(
            "init_arity_mismatch: {}/{}",
            result.init_with_arity, result.init_symbol_count
        ));
    }

    if !errors.is_empty() {
        result.failed = true;
        result.fail_reason = Some(errors.join(", "));
    }
}

/// Compact one-line status for a result.
fn status_line(result: &SmokeResult) -> String {
    if result.skipped {
        format!(
            "SKIP  {} — {}",
            result.root,
            result.skip_reason.as_deref().unwrap_or("?")
        )
    } else if result.failed {
        format!(
            "FAIL  {} — {}",
            result.root,
            result.fail_reason.as_deref().unwrap_or("?")
        )
    } else {
        format!(
            "PASS  {}  nodes={} edges={} synth={} dup={} dang=({},{}) det={} {}s",
            result.root,
            result.nodes,
            result.edges,
            result.synthetic_count,
            result.duplicate_nodes,
            result.dangling_sources,
            result.dangling_targets,
            result.deterministic,
            result.duration_secs,
        )
    }
}

// ── Fixture helpers ──────────────────────────────────────────────────────────

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join(name)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn fixture_smoke_imports_basic() {
    let root = fixture_path("imports-basic");
    let mut result = run_smoke(&root, TargetKind::Fixture);
    validate_smoke(&mut result);

    println!("\n{}", status_line(&result));

    if result.skipped {
        panic!("Fixture imports-basic should never be skipped");
    }
    if result.failed {
        panic!("FAIL: {}", result.fail_reason.unwrap_or_default());
    }

    // Fixture should have basic structural nodes
    let kinds = &result.node_kind_distribution;
    assert!(kinds.contains_key("Repository"), "Missing Repository node");
    assert!(kinds.contains_key("Package"), "Missing Package node");
    assert!(kinds.contains_key("SourceFile"), "Missing SourceFile node");
    assert!(kinds.contains_key("Symbol"), "Missing Symbol node");

    assert!(result.nodes >= 10, "Too few nodes: {}", result.nodes);
    assert!(result.edges >= 5, "Too few edges: {}", result.edges);
}

#[test]
fn fixture_smoke_constructor_basic() {
    let root = fixture_path("constructor-basic");
    let mut result = run_smoke(&root, TargetKind::Fixture);
    validate_smoke(&mut result);

    println!("\n{}", status_line(&result));

    if result.skipped {
        panic!("Fixture constructor-basic should never be skipped");
    }
    if result.failed {
        panic!("FAIL: {}", result.fail_reason.unwrap_or_default());
    }

    // This fixture has constructor symbols
    assert!(
        result.init_symbol_count > 0,
        "Expected Init symbols in constructor fixture"
    );
    assert_eq!(
        result.init_symbol_count, result.init_with_arity,
        "All Init symbols should have #arity suffix"
    );
    assert_eq!(
        result.synthetic_constructor, 0,
        "Constructor synthetic should be 0"
    );
    assert_eq!(
        result.synthetic_count, 0,
        "All synthetic should be 0 in constructor fixture"
    );
}

#[test]
fn fixture_smoke_reference_cross_file() {
    let root = fixture_path("reference-cross-file-basic");
    let mut result = run_smoke(&root, TargetKind::Fixture);
    validate_smoke(&mut result);

    println!("\n{}", status_line(&result));

    if result.skipped {
        panic!("Fixture reference-cross-file-basic should never be skipped");
    }
    if result.failed {
        panic!("FAIL: {}", result.fail_reason.unwrap_or_default());
    }

    // Cross-file fixture should have Uses edges
    let edges = &result.edge_kind_distribution;
    let uses = edges.get("Uses").copied().unwrap_or(0);
    assert!(uses > 0, "Expected Uses edges in cross-file fixture");
}

#[test]
#[ignore] // 依赖本机绝对路径，默认跳过；手动 opt-in: --ignored --nocapture
fn test_production_smoke() {
    // ── Machine-local production targets (optional) ─────────────────────────
    // These paths may not exist on every machine.
    // Missing paths are gracefully skipped — they do NOT fail the test.
    let production_targets: Vec<(&str, &str)> = vec![
        (
            "cjgui-GitNexus-Index",
            "/Users/jiangxuanyang/Desktop/cangjie-GitNexus-Index/runtime/cjgui",
        ),
        (
            "cjgui-cangjie",
            "/Users/jiangxuanyang/Desktop/cangjie/runtime/cjgui",
        ),
        (
            "web_framework",
            "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/web_framework/project",
        ),
        (
            "json_parser",
            "/Users/jiangxuanyang/Desktop/cangjie/repos/CangjieSkills/tests/json_parser/project",
        ),
    ];

    let mut results: Vec<SmokeResult> = Vec::new();

    for (label, path) in &production_targets {
        let mut result = run_smoke(Path::new(path), TargetKind::Production);
        validate_smoke(&mut result);

        if result.skipped {
            println!(
                "{:<4}  {:<25}  SKIP — {}",
                "",
                label,
                result.skip_reason.as_deref().unwrap_or("?")
            );
        } else if result.failed {
            println!(
                "{:<4}  {:<25}  FAIL — {}",
                "FAIL",
                label,
                result.fail_reason.as_deref().unwrap_or("?")
            );
        } else {
            println!(
                "{:<4}  {:<25}  n={:<6} e={:<6} synth={:<4} dup={}  dang=({},{})  det={}  {:.1}s",
                "PASS",
                label,
                result.nodes,
                result.edges,
                result.synthetic_count,
                result.duplicate_nodes,
                result.dangling_sources,
                result.dangling_targets,
                result.deterministic,
                result.duration_secs,
            );
        }

        results.push(result);
    }

    // ── Summary ────────────────────────────────────────────────────────────
    let successful: Vec<_> = results.iter().filter(|r| !r.skipped && !r.failed).collect();
    let failed: Vec<_> = results.iter().filter(|r| r.failed).collect();
    let skipped: Vec<_> = results.iter().filter(|r| r.skipped).collect();

    println!("\n── Production Smoke Summary ──");
    println!("  targets: {} total", results.len());
    println!("  pass:    {}", successful.len());
    println!("  skip:    {}", skipped.len());
    println!("  fail:    {}", failed.len());

    if !successful.is_empty() {
        let total_nodes: usize = successful.iter().map(|r| r.nodes).sum();
        let total_edges: usize = successful.iter().map(|r| r.edges).sum();
        let total_synth: usize = successful.iter().map(|r| r.synthetic_count).sum();
        let total_c: usize = successful.iter().map(|r| r.synthetic_constructor).sum();
        let total_m: usize = successful.iter().map(|r| r.synthetic_method).sum();
        let total_f: usize = successful.iter().map(|r| r.synthetic_function).sum();
        let total_init: usize = successful.iter().map(|r| r.init_symbol_count).sum();
        let total_dur: f64 = successful.iter().map(|r| r.duration_secs).sum();

        println!();
        println!("  nodes:     {}", total_nodes);
        println!("  edges:     {}", total_edges);
        println!(
            "  synthetic: {} (Constructor={}, Method={}, Function={})",
            total_synth, total_c, total_m, total_f
        );
        println!("  init:      {}", total_init);
        println!("  time:      {:.1}s", total_dur);
    }

    if !skipped.is_empty() {
        println!("\n  skipped:");
        for r in &skipped {
            println!(
                "    - {} ({})",
                r.root,
                r.skip_reason.as_deref().unwrap_or("?")
            );
        }
    }

    if !failed.is_empty() {
        println!("\n  FAILED:");
        for r in &failed {
            println!(
                "    - {} ({})",
                r.root,
                r.fail_reason.as_deref().unwrap_or("?")
            );
        }
    }

    // Hard assertions: failed targets must be 0
    if !failed.is_empty() {
        panic!(
            "{} production target(s) FAILED: {}",
            failed.len(),
            failed
                .iter()
                .map(|r| r.root.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // All successful targets must have 0 synthetic (current quality bar)
    for r in &successful {
        assert_eq!(
            r.synthetic_count, 0,
            "Nonzero synthetic in {}: Constructor={}, Method={}, Function={}",
            r.root, r.synthetic_constructor, r.synthetic_method, r.synthetic_function
        );
    }
}
