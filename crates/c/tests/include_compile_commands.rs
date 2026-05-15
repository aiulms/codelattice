//! Integration tests for C compile_commands.json include resolution.
//!
//! Uses the fixtures/c/include-compile-commands fixture.

use gitnexus_c::graph::CGraphEdge;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Helper to get the fixture root path.
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/c/include-compile-commands")
}

#[test]
fn test_parse_command_string_extracts_flags() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).unwrap();

    let main_path = root.join("src/main.c");
    let entry = db.for_file(&main_path).unwrap();

    // -Iinclude
    assert!(
        entry
            .project_include_dirs
            .iter()
            .any(|d| d.ends_with("include")),
        "Expected -Iinclude in project_include_dirs: {:?}",
        entry.project_include_dirs
    );

    // -Igenerated
    assert!(
        entry
            .project_include_dirs
            .iter()
            .any(|d| d.ends_with("generated")),
        "Expected -Igenerated in project_include_dirs: {:?}",
        entry.project_include_dirs
    );

    // -iquote src/internal
    assert!(
        entry
            .quote_include_dirs
            .iter()
            .any(|d| d.ends_with("src/internal")),
        "Expected -iquote src/internal: {:?}",
        entry.quote_include_dirs
    );

    // -DAPP_DEBUG=1
    assert!(
        entry
            .defines
            .iter()
            .any(|(k, v)| k == "APP_DEBUG" && v == &Some("1".to_string())),
        "Expected -DAPP_DEBUG=1: {:?}",
        entry.defines
    );

    // -include generated/version.h
    assert!(
        entry
            .forced_includes
            .iter()
            .any(|p| p.ends_with("version.h")),
        "Expected -include generated/version.h: {:?}",
        entry.forced_includes
    );
}

#[test]
fn test_local_include_resolves_through_include_dir() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let all_files = vec![
        root.join("src/main.c"),
        root.join("src/logger.c"),
        root.join("src/internal/detail.c"),
        root.join("include/app/config.h"),
        root.join("include/app/logger.h"),
        root.join("src/internal/detail.h"),
        root.join("generated/version.h"),
    ];
    let headers: Vec<PathBuf> = all_files
        .iter()
        .filter(|f| f.extension().map(|e| e == "h").unwrap_or(false))
        .cloned()
        .collect();
    let sources: Vec<PathBuf> = all_files
        .iter()
        .filter(|f| f.extension().map(|e| e == "c").unwrap_or(false))
        .cloned()
        .collect();

    let resolver = gitnexus_c::CIncludeResolver::build(&root, &sources, &headers, db);

    // main.c includes "app/logger.h" — should resolve via -Iinclude
    let inc = gitnexus_c::CInclude {
        path: "app/logger.h".to_string(),
        kind: gitnexus_c::CIncludeKind::Local,
        line: 1,
    };
    let resolved = resolver.resolve(&root.join("src/main.c"), &inc);
    assert!(
        resolved.target_file.is_some(),
        "Should resolve app/logger.h"
    );
    assert!(resolved.target_file.as_ref().unwrap().ends_with("logger.h"));
}

#[test]
fn test_quote_include_resolves_through_iquote() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![
        root.join("include/app/config.h"),
        root.join("include/app/logger.h"),
        root.join("src/internal/detail.h"),
        root.join("generated/version.h"),
    ];
    let sources = vec![
        root.join("src/main.c"),
        root.join("src/logger.c"),
        root.join("src/internal/detail.c"),
    ];

    let resolver = gitnexus_c::CIncludeResolver::build(&root, &sources, &headers, db);

    // main.c has -iquote src/internal, include "detail.h" should resolve through it
    let inc = gitnexus_c::CInclude {
        path: "detail.h".to_string(),
        kind: gitnexus_c::CIncludeKind::Local,
        line: 4,
    };
    let resolved = resolver.resolve(&root.join("src/main.c"), &inc);
    assert!(
        resolved.target_file.is_some(),
        "Should resolve detail.h via -iquote"
    );
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_c::CResolvedIncludeKind::QuoteIncludeDir
    );
}

#[test]
fn test_forced_include_resolves() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![
        root.join("include/app/config.h"),
        root.join("include/app/logger.h"),
        root.join("src/internal/detail.h"),
        root.join("generated/version.h"),
    ];
    let sources = vec![
        root.join("src/main.c"),
        root.join("src/logger.c"),
        root.join("src/internal/detail.c"),
    ];

    let resolver = gitnexus_c::CIncludeResolver::build(&root, &sources, &headers, db);

    let forced = resolver.resolve_forced_includes(&root.join("src/main.c"));
    assert!(!forced.is_empty(), "main.c should have forced includes");
    assert!(forced[0]
        .target_file
        .as_ref()
        .unwrap()
        .ends_with("version.h"));
}

#[test]
fn test_system_include_does_not_create_project_edge() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![root.join("include/app/config.h")];
    let sources = vec![root.join("src/main.c")];

    let resolver = gitnexus_c::CIncludeResolver::build(&root, &sources, &headers, db);

    let inc = gitnexus_c::CInclude {
        path: "stdio.h".to_string(),
        kind: gitnexus_c::CIncludeKind::System,
        line: 5,
    };
    let resolved = resolver.resolve(&root.join("src/main.c"), &inc);
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_c::CResolvedIncludeKind::SystemExternal
    );
    assert!(resolved.target_file.is_none());
}

#[test]
fn test_missing_include_creates_diagnostic_and_no_edge() {
    let root = fixture_root();
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let headers = vec![root.join("include/app/config.h")];
    let sources = vec![root.join("src/main.c")];

    let resolver = gitnexus_c::CIncludeResolver::build(&root, &sources, &headers, db);

    let inc = gitnexus_c::CInclude {
        path: "missing.h".to_string(),
        kind: gitnexus_c::CIncludeKind::Local,
        line: 6,
    };
    let resolved = resolver.resolve(&root.join("src/main.c"), &inc);
    assert_eq!(
        resolved.resolution_kind,
        gitnexus_c::CResolvedIncludeKind::Unresolved
    );
    assert!(resolved.target_file.is_none());
}

#[test]
fn test_graph_has_no_dangling_edges() {
    let root = fixture_root();

    // Build project
    let project = gitnexus_c::project::find_c_project_root(&root).unwrap();
    let (source_files, header_files) = gitnexus_c::project::list_c_source_files(&project).unwrap();

    let mut symbols_by_file: BTreeMap<PathBuf, Vec<gitnexus_c::CSymbol>> = BTreeMap::new();
    let mut includes_by_file: BTreeMap<PathBuf, Vec<gitnexus_c::CInclude>> = BTreeMap::new();

    for file in source_files.iter().chain(header_files.iter()) {
        let source = std::fs::read_to_string(file).unwrap();
        let syms = gitnexus_c::extract_c_symbols(&source);
        let incs = gitnexus_c::extract_c_includes(&source);
        if !syms.is_empty() {
            symbols_by_file.insert(file.clone(), syms);
        }
        if !incs.is_empty() {
            includes_by_file.insert(file.clone(), incs);
        }
    }

    // Load compile_commands.json
    let db = gitnexus_c::load_compile_commands(&root.join("compile_commands.json")).ok();
    let resolver = gitnexus_c::CIncludeResolver::build(&root, &source_files, &header_files, db);

    let graph = gitnexus_c::build_c_graph(
        &project,
        &symbols_by_file,
        &includes_by_file,
        Some(&resolver),
    );

    // Collect all node IDs
    let node_ids: std::collections::HashSet<String> =
        graph.nodes.iter().map(|n| n.id.clone()).collect();

    // Check no dangling edges
    let dangling: Vec<&CGraphEdge> = graph
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
}
