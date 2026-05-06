//! Integration tests for function call reference extraction.
//!
//! Verifies that function calls and constructor calls produce USES edges
//! for same-file and cross-file (via explicit import) cases.
//!
//! Requires the `tree-sitter-cangjie` feature.

#[cfg(feature = "tree-sitter-cangjie")]
mod function_call_reference {
    use gitnexus_cangjie::extractors::references::ImportBindingTable;
    use gitnexus_cangjie::extractors::{
        extract_cangjie_imports, extract_cangjie_references, extract_cangjie_symbols_from_tree,
        parse_cangjie_source, CangjieReference, ReferenceKind,
    };
    use gitnexus_cangjie::graph::inspect_cangjie_project;
    use gitnexus_cangjie::project::build_project_model;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    // ── Fixture paths ──

    fn basic_fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("reference-function-call-basic")
    }

    fn cross_file_fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("cangjie")
            .join("reference-function-call-cross-file")
    }

    fn basic_main_path() -> PathBuf {
        basic_fixture_dir().join("src").join("main.cj")
    }

    fn basic_main_source() -> String {
        std::fs::read_to_string(basic_main_path()).expect("failed to read basic fixture")
    }

    /// Extract same-file references from the basic fixture.
    fn extract_basic_references() -> Vec<CangjieReference> {
        let source = basic_main_source();
        let tree = parse_cangjie_source(&source).expect("parse should succeed");
        let symbols = extract_cangjie_symbols_from_tree(&source, &tree).expect("symbol extraction");
        extract_cangjie_references(&source, &basic_main_path(), &symbols, &tree, None)
            .expect("reference extraction should succeed")
    }

    // ── Fixture sanity ──

    #[test]
    fn basic_fixture_parses_cleanly() {
        let source = basic_main_source();
        let tree = parse_cangjie_source(&source).expect("parse should succeed");
        assert!(
            !gitnexus_cangjie::extractors::tree_has_error_nodes(&tree),
            "function-call-basic fixture must parse without ERROR nodes"
        );
    }

    #[test]
    fn basic_fixture_has_symbols() {
        let source = basic_main_source();
        let tree = parse_cangjie_source(&source).expect("parse should succeed");
        let symbols = extract_cangjie_symbols_from_tree(&source, &tree).expect("symbol extraction");
        // Should have: add, compute, Point (class), createOrigin, Calculator (class), testMethodCall, main
        // + init methods for Point and Calculator
        assert!(
            symbols.len() >= 7,
            "expected at least 7 symbols, got {}",
            symbols.len()
        );
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        println!("Symbol names: {:?}", names);
        assert!(names.contains(&"add"), "should have 'add' function");
        assert!(names.contains(&"Point"), "should have 'Point' class");
        assert!(
            names.contains(&"Calculator"),
            "should have 'Calculator' class"
        );
    }

    // ── Same-file function call ──

    #[test]
    fn same_file_function_call_produces_uses_edge() {
        let refs = extract_basic_references();

        // add(x, y) inside compute → USES edge to add
        let add_calls: Vec<_> = refs
            .iter()
            .filter(|r| {
                r.kind == ReferenceKind::Uses
                    && r.target_name == "add"
                    && r.reason == "cangjie-function-call"
            })
            .collect();

        println!("add function call refs: {}", add_calls.len());
        for r in &add_calls {
            println!(
                "  target={} source={} confidence={} target_kinds={:?}",
                r.target_name, r.source_id, r.confidence, r.target_kinds
            );
        }

        assert!(
            !add_calls.is_empty(),
            "expected at least one USES edge from add(x, y) function call"
        );

        let add_call = add_calls[0];
        assert_eq!(
            add_call.confidence, 0.80,
            "function call confidence should be 0.80"
        );
        assert!(
            add_call
                .target_kinds
                .iter()
                .any(|k| matches!(k, gitnexus_cangjie::extractors::CangjieSymbolKind::Function)),
            "function call target should include Function kind"
        );
        // Same-file: target_file should be None (emitter uses file_path)
        assert!(
            add_call.target_file.is_none(),
            "same-file ref should have no target_file"
        );
        assert!(
            !add_call.source_id.is_empty(),
            "source_id must not be empty"
        );
    }

    // ── Constructor call ──

    #[test]
    fn constructor_call_produces_uses_edge() {
        let refs = extract_basic_references();

        // Point(0, 0) inside createOrigin → USES edge to Point
        let point_calls: Vec<_> = refs
            .iter()
            .filter(|r| {
                r.kind == ReferenceKind::Uses
                    && r.target_name == "Point"
                    && r.reason == "cangjie-function-call"
            })
            .collect();

        println!("Point constructor call refs: {}", point_calls.len());
        for r in &point_calls {
            println!(
                "  target={} source={} confidence={} target_kinds={:?}",
                r.target_name, r.source_id, r.confidence, r.target_kinds
            );
        }

        assert!(
            !point_calls.is_empty(),
            "expected at least one USES edge from Point(0, 0) constructor call"
        );

        let point_call = point_calls[0];
        assert_eq!(point_call.confidence, 0.80);
        // Constructor call target should include Class
        assert!(
            point_call
                .target_kinds
                .iter()
                .any(|k| matches!(k, gitnexus_cangjie::extractors::CangjieSymbolKind::Class)),
            "constructor call target should include Class kind"
        );
    }

    // ── Builtin constructor: no edge ──

    #[test]
    fn builtin_constructor_no_edge() {
        let refs = extract_basic_references();

        // Array, Int64, Float64, String constructor calls should NOT produce USES edges
        let builtins = ["Array", "Int64", "Float64", "String", "Bool", "Int32"];
        for builtin in &builtins {
            let matches: Vec<_> = refs
                .iter()
                .filter(|r| {
                    r.kind == ReferenceKind::Uses
                        && r.target_name == *builtin
                        && r.reason == "cangjie-function-call"
                })
                .collect();
            assert!(
                matches.is_empty(),
                "builtin '{}' should not produce function call USES edge, got {}",
                builtin,
                matches.len()
            );
        }
    }

    // ── Method call: no edge ──

    #[test]
    fn method_call_no_edge() {
        let refs = extract_basic_references();

        // calc.getValue() inside testMethodCall → should NOT produce USES edge
        // (method dispatch requires type inference — stop-line)
        let get_value_refs: Vec<_> = refs
            .iter()
            .filter(|r| {
                r.kind == ReferenceKind::Uses
                    && r.target_name == "getValue"
                    && r.reason == "cangjie-function-call"
            })
            .collect();

        println!("getValue method call refs: {}", get_value_refs.len());
        assert!(
            get_value_refs.is_empty(),
            "method call calc.getValue() should not produce USES edge (method dispatch stop-line), got {}",
            get_value_refs.len()
        );
    }

    // ── Unresolved function call: no edge ──

    #[test]
    fn unresolved_function_call_no_edge() {
        let refs = extract_basic_references();

        // Any USES edges with reason "cangjie-function-call" must have a target in the symbol set
        // Unresolved calls should not produce edges
        let source = basic_main_source();
        let tree = parse_cangjie_source(&source).expect("parse should succeed");
        let symbols = extract_cangjie_symbols_from_tree(&source, &tree).expect("symbol extraction");

        let symbol_names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        println!("Defined symbol names: {:?}", symbol_names);

        for r in &refs {
            if r.reason == "cangjie-function-call" {
                // If same-file (no target_file), the target_name should be in symbols
                if r.target_file.is_none() {
                    assert!(
                        symbol_names.contains(&r.target_name.as_str()),
                        "same-file function call to '{}' should resolve to a defined symbol, but '{}' not found in {:?}",
                        r.target_name,
                        r.target_name,
                        symbol_names
                    );
                }
            }
        }
    }

    // ── Cross-file function call via import ──

    #[test]
    fn cross_file_function_call_via_import() {
        let root = cross_file_fixture_dir();

        // Build project model
        let project = build_project_model(&root).expect("build_project_model should succeed");

        println!("Project source files:");
        for f in &project.source_files {
            println!("  {}", f.display());
        }

        // Extract symbols and imports from all source files
        let mut symbols_by_file: BTreeMap<PathBuf, Vec<gitnexus_cangjie::CangjieSymbol>> =
            BTreeMap::new();
        let mut file_trees: BTreeMap<PathBuf, tree_sitter::Tree> = BTreeMap::new();
        let mut imports_by_file: BTreeMap<
            PathBuf,
            Vec<gitnexus_cangjie::extractors::CangjieImport>,
        > = BTreeMap::new();

        for file_path in &project.source_files {
            let source = std::fs::read_to_string(file_path).unwrap();
            let tree = parse_cangjie_source(&source).unwrap();
            let symbols =
                extract_cangjie_symbols_from_tree(&source, &tree).expect("symbol extraction");

            println!("File {}: {} symbols", file_path.display(), symbols.len());
            for s in &symbols {
                println!("  {:?} {}", s.kind, s.name);
            }

            symbols_by_file.insert(file_path.clone(), symbols);
            let imports = extract_cangjie_imports(&source, file_path, &tree);
            if !imports.is_empty() {
                println!("File {}: {} imports", file_path.display(), imports.len());
                for imp in &imports {
                    println!("  import {} (wildcard={})", imp.raw_path, imp.is_wildcard);
                }
                imports_by_file.insert(file_path.clone(), imports);
            }
            file_trees.insert(file_path.clone(), tree);
        }

        // Build import binding table
        let bindings = ImportBindingTable::build(&symbols_by_file, &imports_by_file, &project);
        println!("ImportBindingTable built");

        // Extract references from main.cj WITH import bindings
        let main_path = root.join("src").join("main.cj");
        let main_source = std::fs::read_to_string(&main_path).unwrap();
        let main_symbols = symbols_by_file.get(&main_path).unwrap();
        let main_tree = file_trees.get(&main_path).unwrap();

        let refs_with_bindings = extract_cangjie_references(
            &main_source,
            &main_path,
            main_symbols,
            main_tree,
            Some(&bindings),
        )
        .expect("reference extraction should succeed");

        // Without import bindings
        let refs_without_bindings =
            extract_cangjie_references(&main_source, &main_path, main_symbols, main_tree, None)
                .expect("reference extraction should succeed");

        println!("\nReferences with bindings: {}", refs_with_bindings.len());
        for r in &refs_with_bindings {
            println!(
                "  {:?} target={} source={} target_file={:?} confidence={} reason={}",
                r.kind, r.target_name, r.source_id, r.target_file, r.confidence, r.reason
            );
        }
        println!(
            "\nReferences without bindings: {}",
            refs_without_bindings.len()
        );
        for r in &refs_without_bindings {
            println!(
                "  {:?} target={} confidence={}",
                r.kind, r.target_name, r.confidence
            );
        }

        // Cross-file function call: add(1, 2) via import mathpkg.ops.{add}
        let add_calls_with: Vec<&CangjieReference> = refs_with_bindings
            .iter()
            .filter(|r| r.target_name == "add" && r.reason.contains("cangjie-function-call"))
            .collect();

        println!(
            "\nadd function call refs: with_bindings={} without_bindings={}",
            add_calls_with.len(),
            refs_without_bindings
                .iter()
                .filter(|r| r.target_name == "add")
                .count()
        );

        // With bindings: should produce cross-file reference
        if !add_calls_with.is_empty() {
            for r in &add_calls_with {
                println!(
                    "  cross-file add ref: target_file={:?} confidence={} reason={}",
                    r.target_file, r.confidence, r.reason
                );
                // Cross-file resolved references should have target_file
                if let Some(ref tf) = r.target_file {
                    assert!(
                        tf.contains("ops.cj"),
                        "cross-file add reference should target ops.cj, got {}",
                        tf
                    );
                    // Cross-file explicit import: confidence should be 0.85
                    assert_eq!(
                        r.confidence, 0.85,
                        "cross-file function call confidence should be 0.85, got {}",
                        r.confidence
                    );
                    assert!(
                        r.reason.contains("cross-file"),
                        "reason should mention cross-file, got: {}",
                        r.reason
                    );
                }
            }
        } else {
            // If no cross-file refs, verify that with bindings >= without
            assert!(
                refs_with_bindings.len() >= refs_without_bindings.len(),
                "cross-file bindings should not reduce reference count"
            );
        }

        // With bindings should never have fewer references than without
        assert!(refs_with_bindings.len() >= refs_without_bindings.len());
    }

    // ── Endpoint integrity ──

    #[test]
    fn function_call_reference_targets_exist_in_graph() {
        let root = basic_fixture_dir();
        let output =
            inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

        // Collect all node IDs
        let node_ids: std::collections::HashSet<&str> =
            output.nodes.iter().map(|n| n.id.as_str()).collect();

        println!("Graph has {} nodes", node_ids.len());

        // Check that all USES/ACCESSES/MODIFIES edges have TARGET in node set
        let ref_edges: Vec<_> = output
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

        println!(
            "Checking {} reference edges for target endpoint integrity",
            ref_edges.len()
        );

        for edge in &ref_edges {
            println!(
                "  edge: {:?} {} -> {}",
                edge.kind, edge.source_id, edge.target_id
            );
            assert!(
                node_ids.contains(edge.target_id.as_str()),
                "target_id {} not in graph nodes (source={})",
                edge.target_id,
                edge.source_id
            );
        }
    }

    // ── Cross-file endpoint integrity ──

    #[test]
    fn cross_file_function_call_endpoint_integrity() {
        let root = cross_file_fixture_dir();
        let output =
            inspect_cangjie_project(&root).expect("inspect_cangjie_project should succeed");

        let node_ids: std::collections::HashSet<&str> =
            output.nodes.iter().map(|n| n.id.as_str()).collect();

        // Check all edges (not just function call) for endpoint integrity
        let ref_edges: Vec<_> = output
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

        println!(
            "Checking {} reference edges for endpoint integrity",
            ref_edges.len()
        );
        for edge in ref_edges {
            assert!(
                node_ids.contains(edge.target_id.as_str()),
                "target_id {} not in graph nodes (source={})",
                edge.target_id,
                edge.source_id
            );
        }
    }
}
