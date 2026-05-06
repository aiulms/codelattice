//! Integration tests for Cangjie import resolution.
//!
//! Requires the `tree-sitter-cangjie` feature.
//! Uses the `fixtures/cangjie/imports-basic/` fixture.

#[cfg(feature = "tree-sitter-cangjie")]
mod import_tests {
    use gitnexus_cangjie::extractors::{
        extract_cangjie_imports, extract_cangjie_symbols, parse_cangjie_source,
    };
    use gitnexus_cangjie::graph::inspect_cangjie_project;
    use gitnexus_cangjie::project::find_project_root;
    use std::path::{Path, PathBuf};

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("imports-basic")
    }

    fn read_fixture_file(name: &str) -> String {
        let path = fixture_dir().join("src").join(name);
        std::fs::read_to_string(&path).unwrap()
    }

    /// Parse a fixture source file and return the tree.
    fn parse_fixture_file(name: &str) -> (String, PathBuf, tree_sitter::Tree) {
        let source = read_fixture_file(name);
        let file_path = fixture_dir().join("src").join(name);
        let tree = parse_cangjie_source(&source).unwrap();
        (source, file_path, tree)
    }

    // ---------------------------------------------------------------------------
    // AST parsing tests
    // ---------------------------------------------------------------------------

    #[test]
    fn fixture_main_parses_cleanly() {
        let (_source, _file_path, tree) = parse_fixture_file("main.cj");
        // Walk tree to check for ERROR kind
        fn has_error_node(node: tree_sitter::Node) -> bool {
            if node.kind() == "ERROR" {
                return true;
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i.try_into().unwrap()) {
                    if has_error_node(child) {
                        return true;
                    }
                }
            }
            false
        }
        assert!(!has_error_node(tree.root_node()), "main.cj has ERROR nodes");
    }

    #[test]
    fn fixture_add_parses_cleanly() {
        let source = read_fixture_file("demo/math/add.cj");
        let file_path = fixture_dir().join("src").join("demo/math/add.cj");
        let tree = parse_cangjie_source(&source).unwrap();
        fn has_error_node(node: tree_sitter::Node) -> bool {
            if node.kind() == "ERROR" {
                return true;
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i.try_into().unwrap()) {
                    if has_error_node(child) {
                        return true;
                    }
                }
            }
            false
        }
        assert!(!has_error_node(tree.root_node()), "add.cj has ERROR nodes");
    }

    // ---------------------------------------------------------------------------
    // Import extraction tests
    // ---------------------------------------------------------------------------

    #[test]
    fn imports_are_extracted() {
        let (source, file_path, tree) = parse_fixture_file("main.cj");
        let imports = extract_cangjie_imports(&source, &file_path, &tree);
        // main.cj has: import demo.math.add, import demo.math.{sub, mul},
        // import demo.math.*, import demo.math.Calculator as Calc,
        // public import demo.math.add
        assert!(!imports.is_empty(), "Should extract imports");
        // At least 5 imports
        assert!(
            imports.len() >= 5,
            "Expected >= 5 imports, got {}",
            imports.len()
        );
    }

    #[test]
    fn single_import_has_correct_path() {
        let (source, file_path, tree) = parse_fixture_file("main.cj");
        let imports = extract_cangjie_imports(&source, &file_path, &tree);
        let single = imports
            .iter()
            .find(|i| i.raw_path.contains("demo.math.add") && !i.is_wildcard);
        assert!(single.is_some(), "Should find single import demo.math.add");
    }

    #[test]
    fn wildcard_import_detected() {
        let (source, file_path, tree) = parse_fixture_file("main.cj");
        let imports = extract_cangjie_imports(&source, &file_path, &tree);
        let wildcard = imports.iter().find(|i| i.is_wildcard);
        assert!(
            wildcard.is_some(),
            "Should detect wildcard import (demo.math.*)"
        );
        assert!(wildcard.unwrap().raw_path.contains("*"));
    }

    #[test]
    fn public_import_detected() {
        let (source, file_path, tree) = parse_fixture_file("main.cj");
        let imports = extract_cangjie_imports(&source, &file_path, &tree);
        let public_import = imports
            .iter()
            .find(|i| i.visibility == gitnexus_cangjie::ImportVisibility::Public);
        assert!(public_import.is_some(), "Should detect public import");
    }

    // ---------------------------------------------------------------------------
    // Graph integration tests
    // ---------------------------------------------------------------------------

    #[test]
    fn full_project_graph_contains_import_edges() {
        let root = fixture_dir();
        let project = find_project_root(&root);
        assert!(project.is_some(), "Should find cjpm.toml");

        let output = inspect_cangjie_project(&root).unwrap();

        // Check that we have Imports edges
        let import_edges: Vec<_> = output
            .edges
            .iter()
            .filter(|e| e.kind == gitnexus_cangjie::graph::EdgeKind::Imports)
            .collect();
        // At minimum we should have some import edges
        // (exact count depends on resolution strategy)
        assert!(
            !import_edges.is_empty(),
            "Should have at least some Imports edges"
        );
    }

    #[test]
    fn project_graph_contains_all_node_types() {
        let root = fixture_dir();
        let output = inspect_cangjie_project(&root).unwrap();

        let has_repo = output.nodes.iter().any(|n| n.label == "cangjie-repo");
        let has_pkg = output.nodes.iter().any(|n| n.label == "imports-basic");
        let has_file = output.nodes.iter().any(|n| n.label == "main.cj");
        let has_symbol = output.nodes.iter().any(|n| n.label == "add");

        assert!(has_repo, "Should have repository node");
        assert!(has_pkg, "Should have package node");
        assert!(has_file, "Should have source file node");
        assert!(has_symbol, "Should have symbol nodes");
    }

    #[test]
    fn import_edges_have_valid_structure() {
        let root = fixture_dir();
        let output = inspect_cangjie_project(&root).unwrap();

        for edge in &output.edges {
            // Every edge must have non-empty source and target IDs
            assert!(!edge.source_id.is_empty(), "Edge source_id is empty");
            assert!(!edge.target_id.is_empty(), "Edge target_id is empty");
        }

        // Every Imports edge source should be a file node
        for edge in output
            .edges
            .iter()
            .filter(|e| e.kind == gitnexus_cangjie::graph::EdgeKind::Imports)
        {
            assert!(
                edge.source_id.starts_with("file:"),
                "Imports edge source should be file:, got {}",
                edge.source_id
            );
        }
    }

    #[test]
    fn output_is_deterministic() {
        let root = fixture_dir();
        let a = inspect_cangjie_project(&root).unwrap();
        let b = inspect_cangjie_project(&root).unwrap();

        let json_a = serde_json::to_string_pretty(&a).unwrap();
        let json_b = serde_json::to_string_pretty(&b).unwrap();
        assert_eq!(json_a, json_b, "Output should be deterministic");
    }
}
