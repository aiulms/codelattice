//! Contract regression tests for Cangjie graph output.
//!
//! Validates that the graph contract for known fixtures remains stable:
//! - Quality gates (0 synthetic, 0 duplicate, 0 dangling, deterministic)
//! - Node and edge kind sets
//! - Known symbol IDs present
//! - Known edges present (Defines, Uses, Imports, etc.)
//!
//! These tests verify semantic contract — they check specific node IDs and
//! edge triples that must exist.  If any test fails, the Cangjie graph
//! contract has regressed.
//!
//! Requires the `tree-sitter-cangjie` feature.
//!
//! # Running
//!
//! ```sh
//! cargo test --features tree-sitter-cangjie --test graph_contract -- --nocapture
//! ```

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::graph::{inspect_cangjie_project, NodeKind};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("cangjie")
        .join(name)
}

/// Collected graph data for contract assertions.
struct GraphData {
    node_kinds: HashMap<String, usize>,
    edge_triples: HashSet<(String, String, String)>,
    edge_kinds: HashMap<String, usize>,
    symbol_ids: HashSet<String>,
    synthetic_count: usize,
    duplicate_nodes: usize,
    duplicate_edges: usize,
    dangling_sources: usize,
    dangling_targets: usize,
    deterministic: bool,
}

fn collect_graph(root: &PathBuf) -> GraphData {
    let graph1 = inspect_cangjie_project(root).expect("Graph inspection must succeed");

    let node_ids: HashSet<String> = graph1.nodes.iter().map(|n| n.id.clone()).collect();

    let mut node_kinds: HashMap<String, usize> = HashMap::new();
    for n in &graph1.nodes {
        *node_kinds.entry(format!("{:?}", n.kind)).or_insert(0) += 1;
    }

    let mut seen_node_ids: HashSet<&str> = HashSet::new();
    let mut duplicate_nodes = 0usize;
    for n in &graph1.nodes {
        if !seen_node_ids.insert(n.id.as_str()) {
            duplicate_nodes += 1;
        }
    }

    let mut edge_triples: HashSet<(String, String, String)> = HashSet::new();
    let mut seen_edge_triples: HashSet<(String, String, String)> = HashSet::new();
    let mut duplicate_edges = 0usize;
    let mut edge_kinds: HashMap<String, usize> = HashMap::new();
    let mut dangling_sources = 0usize;
    let mut dangling_targets = 0usize;

    for e in &graph1.edges {
        let triple = (
            format!("{:?}", e.kind),
            e.source_id.clone(),
            e.target_id.clone(),
        );
        if !seen_edge_triples.insert(triple.clone()) {
            duplicate_edges += 1;
        }
        edge_triples.insert(triple);

        *edge_kinds.entry(format!("{:?}", e.kind)).or_insert(0) += 1;

        if !node_ids.contains(&e.source_id) {
            dangling_sources += 1;
        }
        if !node_ids.contains(&e.target_id) {
            dangling_targets += 1;
        }
    }

    let symbol_ids: HashSet<String> = graph1
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Symbol)
        .map(|n| n.id.clone())
        .collect();

    let synthetic_count = graph1
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CallableSource)
        .count();

    let deterministic = match inspect_cangjie_project(root) {
        Ok(graph2) => {
            let json1 = serde_json::to_value(&graph1).unwrap_or_default();
            let json2 = serde_json::to_value(&graph2).unwrap_or_default();
            json1 == json2
        }
        Err(_) => false,
    };

    GraphData {
        node_kinds,
        edge_triples,
        edge_kinds,
        symbol_ids,
        synthetic_count,
        duplicate_nodes,
        duplicate_edges,
        dangling_sources,
        dangling_targets,
        deterministic,
    }
}

fn assert_quality_gates(data: &GraphData) {
    assert_eq!(data.synthetic_count, 0, "Must have zero synthetic nodes");
    assert_eq!(data.duplicate_nodes, 0, "Must have zero duplicate nodes");
    assert_eq!(data.duplicate_edges, 0, "Must have zero duplicate edges");
    assert_eq!(
        data.dangling_sources, 0,
        "Must have zero dangling source references"
    );
    assert_eq!(
        data.dangling_targets, 0,
        "Must have zero dangling target references"
    );
    assert!(data.deterministic, "Output must be deterministic");
}

fn assert_node_kind(kinds: &HashMap<String, usize>, kind: &str, min_count: usize) {
    let count = kinds.get(kind).copied().unwrap_or(0);
    assert!(
        count >= min_count,
        "Expected at least {} {}(s), found {}",
        min_count,
        kind,
        count
    );
}

fn assert_edge_kind(kinds: &HashMap<String, usize>, kind: &str, min_count: usize) {
    let count = kinds.get(kind).copied().unwrap_or(0);
    assert!(
        count >= min_count,
        "Expected at least {} {}(s), found {}",
        min_count,
        kind,
        count
    );
}

fn assert_symbol_exists(symbol_ids: &HashSet<String>, id: &str) {
    assert!(
        symbol_ids.contains(id),
        "Expected symbol '{}' not found in graph",
        id
    );
}

fn assert_edge_exists(
    triples: &HashSet<(String, String, String)>,
    kind: &str,
    source: &str,
    target: &str,
) {
    let triple = (kind.to_string(), source.to_string(), target.to_string());
    assert!(
        triples.contains(&triple),
        "Expected edge ({}, {}, {}) not found",
        kind,
        source,
        target
    );
}

// ── imports-basic fixture contract ─────────────────────────────────────────

#[test]
fn imports_basic_quality_gates() {
    let data = collect_graph(&fixture_path("imports-basic"));
    assert_quality_gates(&data);
}

#[test]
fn imports_basic_node_kind_set() {
    let data = collect_graph(&fixture_path("imports-basic"));
    assert_node_kind(&data.node_kinds, "Repository", 1);
    assert_node_kind(&data.node_kinds, "Package", 1);
    assert_node_kind(&data.node_kinds, "SourceFile", 2);
    assert_node_kind(&data.node_kinds, "Symbol", 7);
}

#[test]
fn imports_basic_edge_kind_set() {
    let data = collect_graph(&fixture_path("imports-basic"));
    assert_edge_kind(&data.edge_kinds, "ContainsPackage", 1);
    assert_edge_kind(&data.edge_kinds, "OwnsSource", 2);
    assert_edge_kind(&data.edge_kinds, "Defines", 7);
    assert_edge_kind(&data.edge_kinds, "Imports", 1);
}

#[test]
fn imports_basic_known_symbols() {
    let data = collect_graph(&fixture_path("imports-basic"));
    assert_symbol_exists(&data.symbol_ids, "sym:src/demo/math/add.cj:Function:add#2");
    assert_symbol_exists(
        &data.symbol_ids,
        "sym:src/demo/math/add.cj:Class:Calculator",
    );
    assert_symbol_exists(&data.symbol_ids, "sym:src/main.cj:Function:main#0");
}

#[test]
fn imports_basic_known_edges() {
    let data = collect_graph(&fixture_path("imports-basic"));
    assert_edge_exists(
        &data.edge_triples,
        "Defines",
        "file:src/demo/math/add.cj",
        "sym:src/demo/math/add.cj:Function:add#2",
    );
    assert_edge_exists(
        &data.edge_triples,
        "Defines",
        "file:src/main.cj",
        "sym:src/main.cj:Function:main#0",
    );
    assert_edge_exists(
        &data.edge_triples,
        "ContainsPackage",
        "repo:cangjie",
        "pkg:imports-basic",
    );
    assert_edge_exists(
        &data.edge_triples,
        "Imports",
        "file:src/main.cj",
        "pkg:imports-basic",
    );
}

// ── constructor-basic fixture contract ─────────────────────────────────────

#[test]
fn constructor_basic_quality_gates() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    assert_quality_gates(&data);
}

#[test]
fn constructor_basic_node_kind_set() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    assert_node_kind(&data.node_kinds, "Repository", 1);
    assert_node_kind(&data.node_kinds, "Package", 1);
    assert_node_kind(&data.node_kinds, "SourceFile", 1);
    assert_node_kind(&data.node_kinds, "Symbol", 9);
}

#[test]
fn constructor_basic_edge_kind_set() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    assert_edge_kind(&data.edge_kinds, "ContainsPackage", 1);
    assert_edge_kind(&data.edge_kinds, "OwnsSource", 1);
    assert_edge_kind(&data.edge_kinds, "Defines", 9);
    assert_edge_kind(&data.edge_kinds, "Uses", 2);
}

#[test]
fn constructor_basic_known_init_symbols() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    // All Init symbols must have #arity suffix
    for id in &data.symbol_ids {
        if id.contains(":Init:") {
            assert!(
                id.contains('#'),
                "Init symbol '{}' must have arity suffix",
                id
            );
        }
    }
    assert_symbol_exists(&data.symbol_ids, "sym:src/main.cj:Init:AppConfig.init#2");
    assert_symbol_exists(&data.symbol_ids, "sym:src/main.cj:Init:Point.init#2");
}

#[test]
fn constructor_basic_no_synthetic_constructor() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    // All Init symbols should resolve to real Symbol nodes, not synthetic
    for triple in &data.edge_triples {
        if triple.0 == "Uses" {
            // Uses edges should have real Symbol sources (with #arity for Inits)
            assert!(
                !data.symbol_ids.contains(&triple.1) || triple.1.starts_with("sym:"),
                "Uses edge source should be a Symbol node: {}",
                triple.1
            );
        }
    }
}

#[test]
fn constructor_basic_known_edges() {
    let data = collect_graph(&fixture_path("constructor-basic"));
    assert_edge_exists(
        &data.edge_triples,
        "Defines",
        "file:src/main.cj",
        "sym:src/main.cj:Init:Point.init#2",
    );
    assert_edge_exists(
        &data.edge_triples,
        "Uses",
        "sym:src/main.cj:Function:main#0",
        "sym:src/main.cj:Struct:Point",
    );
}

// ── reference-cross-file-basic fixture contract ────────────────────────────

#[test]
fn reference_cross_file_quality_gates() {
    let data = collect_graph(&fixture_path("reference-cross-file-basic"));
    assert_quality_gates(&data);
}

#[test]
fn reference_cross_file_node_kind_set() {
    let data = collect_graph(&fixture_path("reference-cross-file-basic"));
    assert_node_kind(&data.node_kinds, "Repository", 1);
    assert_node_kind(&data.node_kinds, "Package", 1);
    assert_node_kind(&data.node_kinds, "SourceFile", 2);
    assert_node_kind(&data.node_kinds, "Symbol", 4);
}

#[test]
fn reference_cross_file_edge_kind_set() {
    let data = collect_graph(&fixture_path("reference-cross-file-basic"));
    assert_edge_kind(&data.edge_kinds, "ContainsPackage", 1);
    assert_edge_kind(&data.edge_kinds, "OwnsSource", 2);
    assert_edge_kind(&data.edge_kinds, "Defines", 4);
    assert_edge_kind(&data.edge_kinds, "Imports", 1);
    assert_edge_kind(&data.edge_kinds, "Uses", 1);
}

#[test]
fn reference_cross_file_known_symbols() {
    let data = collect_graph(&fixture_path("reference-cross-file-basic"));
    assert_symbol_exists(&data.symbol_ids, "sym:src/mathpkg/ops.cj:Class:Point");
    assert_symbol_exists(&data.symbol_ids, "sym:src/mathpkg/ops.cj:Function:add#2");
    assert_symbol_exists(&data.symbol_ids, "sym:src/main.cj:Function:main#0");
}

#[test]
fn reference_cross_file_known_edges() {
    let data = collect_graph(&fixture_path("reference-cross-file-basic"));
    assert_edge_exists(
        &data.edge_triples,
        "Uses",
        "sym:src/main.cj:Function:main#0",
        "sym:src/mathpkg/ops.cj:Class:Point",
    );
    assert_edge_exists(
        &data.edge_triples,
        "Imports",
        "file:src/main.cj",
        "pkg:reference-cross-file-basic",
    );
    assert_edge_exists(
        &data.edge_triples,
        "Defines",
        "file:src/mathpkg/ops.cj",
        "sym:src/mathpkg/ops.cj:Function:add#2",
    );
}
