//! Crate-level tests for TYPE_USE edge target resolution.
//!
//! Uses the `portable-smoke` fixture at `fixtures/typescript/portable-smoke/`.
//! 覆盖三个场景：跨文件 import 的类型引用、同文件类型引用、未解析类型
//! （不出边、只记诊断）。这是 graph schema v0.2 "CALLS edge must not be
//! dangling" stop-line 在 TYPE_USE 边上的等价约束。

use std::collections::BTreeMap;
use std::path::PathBuf;

use gitnexus_typescript::graph::{build_ts_graph, TsEdgeKind, TsGraphEdge};
use gitnexus_typescript::TsModuleResolver;

/// Helper: get the fixture root path.
fn fixture_root() -> PathBuf {
    let mut path = std::env::current_dir().expect("current dir");
    for _ in 0..5 {
        if path.join("fixtures/typescript/portable-smoke").is_dir() {
            return path.join("fixtures/typescript/portable-smoke");
        }
        if !path.pop() {
            break;
        }
    }
    std::env::current_dir()
        .expect("current dir")
        .join("fixtures/typescript/portable-smoke")
}

/// Build the graph for the fixture the same way the CLI does.
#[cfg(feature = "tree-sitter-typescript")]
fn build_fixture_graph() -> gitnexus_typescript::graph::TsGraphOutput {
    let root = fixture_root();
    let project =
        gitnexus_typescript::find_typescript_project_root(&root).expect("find project root");
    let source_files = gitnexus_typescript::list_source_files(&project).expect("list source files");
    let manifest = gitnexus_typescript::load_ts_manifest(&project).ok();
    let kind = gitnexus_typescript::project::detect_project_kind(&project);
    let ts_project = gitnexus_typescript::TsProject {
        root: project,
        kind,
        manifest,
        source_files: source_files.clone(),
    };

    let mut symbols_by_file: BTreeMap<PathBuf, Vec<gitnexus_typescript::TsSymbol>> =
        BTreeMap::new();
    let mut imports_by_file: BTreeMap<PathBuf, Vec<gitnexus_typescript::TsImport>> =
        BTreeMap::new();
    let mut references_by_file: BTreeMap<PathBuf, Vec<gitnexus_typescript::TsReference>> =
        BTreeMap::new();

    for file in &source_files {
        let source = std::fs::read_to_string(file).expect("read source file");
        let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
            gitnexus_typescript::extractors::TsLanguage::Tsx
        } else {
            gitnexus_typescript::extractors::TsLanguage::TypeScript
        };
        let extraction = gitnexus_typescript::extractors::extract_ts_file(&source, lang);
        symbols_by_file.insert(file.clone(), extraction.symbols);
        imports_by_file.insert(file.clone(), extraction.imports);
        references_by_file.insert(file.clone(), extraction.references);
    }

    let resolver = TsModuleResolver::build(&ts_project.root, &source_files);
    build_ts_graph(
        &ts_project,
        &symbols_by_file,
        &imports_by_file,
        &references_by_file,
        Some(&resolver),
    )
}

#[cfg(feature = "tree-sitter-typescript")]
fn type_use_edges(graph: &gitnexus_typescript::graph::TsGraphOutput) -> Vec<&TsGraphEdge> {
    graph
        .edges
        .iter()
        .filter(|e| e.kind == TsEdgeKind::TypeUse)
        .collect()
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn type_use_edges_never_dangle() {
    let graph = build_fixture_graph();
    let node_ids: std::collections::BTreeSet<&str> =
        graph.nodes.iter().map(|n| n.id.as_str()).collect();

    let type_uses = type_use_edges(&graph);
    assert!(
        !type_uses.is_empty(),
        "fixture should produce at least one TYPE_USE edge"
    );

    for edge in &type_uses {
        assert!(
            !edge.target.starts_with("ref:"),
            "TYPE_USE target must be a real node, got synthetic ref: {}",
            edge.target
        );
        assert!(
            node_ids.contains(edge.target.as_str()),
            "TYPE_USE target {} must exist in nodes",
            edge.target
        );
        if let Some(source) = &edge.source {
            assert!(
                node_ids.contains(source.as_str()),
                "TYPE_USE source {} must exist in nodes",
                source
            );
        }
    }
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn imported_and_same_file_type_uses_resolve_with_confidence() {
    let graph = build_fixture_graph();
    let type_uses = type_use_edges(&graph);

    // index.ts 里 `import type { Shape, Point } from "./model"` 后使用
    // Point/Shape 注解 —— 应解析到 model.ts 的真实符号，reason=imported-type。
    let imported: Vec<&TsGraphEdge> = type_uses
        .iter()
        .filter(|e| edge_reason(e) == Some("imported-type"))
        .copied()
        .collect();
    assert!(
        imported.iter().any(|e| e.target.contains("Point")),
        "Point (type alias in model.ts) should resolve via import binding"
    );
    assert!(
        imported.iter().any(|e| e.target.contains("Shape")),
        "Shape (interface in model.ts) should resolve via import binding"
    );

    // index.ts 同文件定义的 User 接口 —— reason=same-file-type。
    let same_file: Vec<&TsGraphEdge> = type_uses
        .iter()
        .filter(|e| edge_reason(e) == Some("same-file-type"))
        .copied()
        .collect();
    assert!(
        same_file.iter().any(|e| e.target.contains("User")),
        "User (interface in index.ts) should resolve as same-file type"
    );

    // 所有解析成功的 TYPE_USE 边都必须携带数值 confidence（解析边口径依赖它）。
    for edge in &type_uses {
        let props = edge.properties.as_ref().expect("TYPE_USE edge properties");
        assert!(
            props.get("confidence").and_then(|c| c.as_f64()).is_some(),
            "resolved TYPE_USE edge must carry numeric confidence: {}",
            edge.target
        );
        assert!(
            props.get("useCount").and_then(|c| c.as_u64()).unwrap_or(0) >= 1,
            "resolved TYPE_USE edge must carry useCount >= 1: {}",
            edge.target
        );
    }
}

#[cfg(feature = "tree-sitter-typescript")]
#[test]
fn unresolved_type_uses_emit_diagnostics_not_edges() {
    let graph = build_fixture_graph();
    let node_ids: std::collections::BTreeSet<&str> =
        graph.nodes.iter().map(|n| n.id.as_str()).collect();

    // 不变量：图里不允许任何 dangling 边（不只 TYPE_USE）。
    for edge in &graph.edges {
        if let Some(source) = &edge.source {
            assert!(
                node_ids.contains(source.as_str()),
                "edge source {} must exist in nodes",
                source
            );
        }
        assert!(
            node_ids.contains(edge.target.as_str()),
            "edge target {} must exist in nodes",
            edge.target
        );
    }

    // 若存在未解析类型引用，只能以诊断形式出现，且诊断被 file+name 去重。
    let unresolved: Vec<&serde_json::Value> = graph
        .diagnostics
        .iter()
        .filter(|d| {
            d.get("kind").and_then(|k| k.as_str()) == Some("typescript-type-use-unresolved")
        })
        .collect();
    let mut seen: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
    for diag in &unresolved {
        let source = diag.get("source").and_then(|s| s.as_str()).unwrap_or("");
        let name = diag.get("name").and_then(|s| s.as_str()).unwrap_or("");
        assert!(
            seen.insert((source.to_string(), name.to_string())),
            "unresolved type-use diagnostics must be deduped by file+name: {} {}",
            source,
            name
        );
    }
}

#[cfg(feature = "tree-sitter-typescript")]
fn edge_reason(edge: &TsGraphEdge) -> Option<&str> {
    edge.properties
        .as_ref()
        .and_then(|p| p.get("reason"))
        .and_then(|r| r.as_str())
}
