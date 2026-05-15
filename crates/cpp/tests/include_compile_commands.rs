//! Integration tests for C++ compile_commands.json include resolution.
//!
//! Uses the fixtures/cpp/include-compile-commands fixture.

use gitnexus_cpp::graph::CppGraphEdge;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Helper to get the fixture root path.
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/cpp/include-compile-commands")
}

#[test]
fn test_parse_arguments_array_extracts_flags() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).unwrap();

    let main_path = root.join("src/main.cpp");
    let entry = db.for_file(&main_path).unwrap();

    // -I include (separate arg)
    assert!(
        entry
            .project_include_dirs
            .iter()
            .any(|d| d.ends_with("include")),
        "Expected -I include in project_include_dirs: {:?}",
        entry.project_include_dirs
    );

    // -isystem third_party/include
    assert!(
        entry
            .system_include_dirs
            .iter()
            .any(|d| d.ends_with("third_party/include")),
        "Expected -isystem third_party/include: {:?}",
        entry.system_include_dirs
    );

    // -iquote src/detail
    assert!(
        entry
            .quote_include_dirs
            .iter()
            .any(|d| d.ends_with("src/detail")),
        "Expected -iquote src/detail: {:?}",
        entry.quote_include_dirs
    );

    // -DAPP_VERSION=1
    assert!(
        entry
            .defines
            .iter()
            .any(|(k, v)| k == "APP_VERSION" && v == &Some("1".to_string())),
        "Expected -DAPP_VERSION=1: {:?}",
        entry.defines
    );

    // -include generated/build_config.hpp
    assert!(
        entry
            .forced_includes
            .iter()
            .any(|p| p.ends_with("build_config.hpp")),
        "Expected -include generated/build_config.hpp: {:?}",
        entry.forced_includes
    );
}

#[test]
fn test_local_include_resolves_through_include_dir() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![
        root.join("include/app/logger.hpp"),
        root.join("include/app/math.hpp"),
        root.join("src/detail/detail.hpp"),
        root.join("generated/build_config.hpp"),
    ];
    let sources = vec![
        root.join("src/main.cpp"),
        root.join("src/logger.cpp"),
        root.join("src/detail/detail.cpp"),
    ];

    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &sources, &headers, db);

    let inc = gitnexus_cpp::CppInclude {
        path: "app/logger.hpp".to_string(),
        kind: gitnexus_cpp::CppIncludeKind::Local,
        line: 1,
    };
    let resolved = resolver.resolve(&root.join("src/main.cpp"), &inc);
    assert!(
        resolved.target_file.is_some(),
        "Should resolve app/logger.hpp"
    );
    assert!(resolved
        .target_file
        .as_ref()
        .unwrap()
        .ends_with("logger.hpp"));
}

#[test]
fn test_quote_include_resolves_through_iquote() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![
        root.join("include/app/logger.hpp"),
        root.join("src/detail/detail.hpp"),
    ];
    let sources = vec![
        root.join("src/main.cpp"),
        root.join("src/detail/detail.cpp"),
    ];

    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &sources, &headers, db);

    // main.cpp has -iquote src/detail, include "detail.hpp" should resolve through it
    let inc = gitnexus_cpp::CppInclude {
        path: "detail.hpp".to_string(),
        kind: gitnexus_cpp::CppIncludeKind::Local,
        line: 3,
    };
    let resolved = resolver.resolve(&root.join("src/main.cpp"), &inc);
    assert!(
        resolved.target_file.is_some(),
        "Should resolve detail.hpp via -iquote"
    );
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_cpp::CppResolvedIncludeKind::QuoteIncludeDir
    );
}

#[test]
fn test_system_include_stays_external() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![root.join("include/app/logger.hpp")];
    let sources = vec![root.join("src/main.cpp")];

    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &sources, &headers, db);

    let inc = gitnexus_cpp::CppInclude {
        path: "vector".to_string(),
        kind: gitnexus_cpp::CppIncludeKind::System,
        line: 4,
    };
    let resolved = resolver.resolve(&root.join("src/main.cpp"), &inc);
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_cpp::CppResolvedIncludeKind::SystemExternal
    );
    assert!(resolved.target_file.is_none());
}

#[test]
fn test_forced_include_resolves() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![
        root.join("include/app/logger.hpp"),
        root.join("generated/build_config.hpp"),
    ];
    let sources = vec![root.join("src/main.cpp")];

    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &sources, &headers, db);

    let forced = resolver.resolve_forced_includes(&root.join("src/main.cpp"));
    assert!(!forced.is_empty(), "main.cpp should have forced includes");
    assert!(forced[0]
        .target_file
        .as_ref()
        .unwrap()
        .ends_with("build_config.hpp"));
}

#[test]
fn test_missing_include_diagnostic() {
    let root = fixture_root();
    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![root.join("include/app/logger.hpp")];
    let sources = vec![root.join("src/main.cpp")];

    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &sources, &headers, db);

    let inc = gitnexus_cpp::CppInclude {
        path: "missing.hpp".to_string(),
        kind: gitnexus_cpp::CppIncludeKind::Local,
        line: 6,
    };
    let resolved = resolver.resolve(&root.join("src/main.cpp"), &inc);
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_cpp::CppResolvedIncludeKind::Unresolved
    );
    assert!(resolved.target_file.is_none());
}

#[test]
fn test_graph_has_include_edges_and_no_dangling_edges() {
    let root = fixture_root();

    let project = gitnexus_cpp::project::find_cpp_project_root(&root).unwrap();
    let (source_files, header_files) =
        gitnexus_cpp::project::list_cpp_source_files(&project).unwrap();

    let mut symbols_by_file: BTreeMap<PathBuf, Vec<gitnexus_cpp::CppSymbol>> = BTreeMap::new();
    let mut includes_by_file: BTreeMap<PathBuf, Vec<gitnexus_cpp::CppInclude>> = BTreeMap::new();
    let mut calls_by_file: BTreeMap<PathBuf, Vec<gitnexus_cpp::CppCall>> = BTreeMap::new();

    let project_fn_names: Vec<String> = Vec::new();

    for file in source_files.iter().chain(header_files.iter()) {
        let source = std::fs::read_to_string(file).unwrap();
        let syms = gitnexus_cpp::extract_cpp_symbols(&source);
        let incs = gitnexus_cpp::extract_cpp_includes(&source);
        let rel = file.strip_prefix(&root).unwrap_or(file);
        let calls =
            gitnexus_cpp::extract_cpp_calls(&source, &rel.to_string_lossy(), &project_fn_names);
        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !incs.is_empty() {
            includes_by_file.insert(file.clone(), incs);
        }
        if !calls.is_empty() {
            calls_by_file.insert(file.clone(), calls);
        }
    }

    let db = gitnexus_cpp::load_compile_commands(&root.join("compile_commands.json")).ok();
    let resolver = gitnexus_cpp::CppIncludeResolver::build(&root, &source_files, &header_files, db);

    let graph = gitnexus_cpp::build_cpp_graph(
        &project,
        &symbols_by_file,
        &includes_by_file,
        &calls_by_file,
        Some(&resolver),
    );

    // Collect all node IDs
    let node_ids: std::collections::HashSet<String> =
        graph.nodes.iter().map(|n| n.id.clone()).collect();

    // Check no dangling edges (including no unresolved: synthetic targets)
    let dangling: Vec<&CppGraphEdge> = graph
        .edges
        .iter()
        .filter(|e| !node_ids.contains(&e.target))
        .collect();

    assert!(
        dangling.is_empty(),
        "Graph has {} dangling edges: {:?}",
        dangling.len(),
        dangling.iter().map(|e| &e.target).collect::<Vec<_>>()
    );

    // Verify there are include edges
    let include_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.kind == gitnexus_cpp::CppEdgeKind::Includes)
        .collect();

    assert!(!include_edges.is_empty(), "Graph should have include edges");
}
